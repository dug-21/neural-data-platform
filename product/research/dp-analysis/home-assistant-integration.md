# Home Assistant Integration - Implementation Plan

## AIR-008: Home Events Data Integration

### Executive Summary

This document outlines the Domain Adapter pattern implementation for integrating Home Assistant data (window open/close states and other home events) into the Neural Data Platform. The design follows existing NDP patterns while addressing the unique characteristics of event-based home automation data.

---

## 1. Architecture Overview

### 1.1 Data Layer Mapping

```
+------------------+    +------------------+    +------------------+
|   Home Assistant |    |    Bronze Layer  |    |   Silver Layer   |
|                  |--->|    (Parquet)     |--->|   (TimescaleDB)  |
|  - Window States |    |  - Raw events    |    |  - State series  |
|  - Door States   |    |  - JSON metadata |    |  - Aggregations  |
|  - Motion Events |    |  - Full context  |    |  - Joins w/ AQ   |
+------------------+    +------------------+    +------------------+
         |                       |                       |
         v                       v                       v
   WebSocket/REST          Daily Partitions       Continuous Aggs
   State Changes            by stream_id           Feature Views
```

### 1.2 NDP Domain Adapter Pattern

Following ADR-001 (Channel Ownership), the integration follows hexagonal architecture:

```
+----------------------------------------------------------+
|                    NDP Core Library                       |
|  +----------------+   +----------------+   +------------+ |
|  |   Source       |   | TimeSeriesPoint|   |   Store    | |
|  |   trait        |   |   struct       |   |   trait    | |
|  +-------+--------+   +-------+--------+   +-----+------+ |
|          |                    |                  |        |
+----------|--------------------|-----------------+|--------+
           |                    |                  |
           v                    v                  v
+------------------+    +---------------+   +-------------+
| HomeAssistant    |    | Point with    |   | ParquetStore|
| Source           |--->| entity_id,    |-->| (Bronze)    |
| (implements      |    | state, attrs  |   +-------------+
|  Source trait)   |    +---------------+          |
+------------------+                               v
                                           +-------------+
                                           |TimescaleDB  |
                                           | (Silver)    |
                                           +-------------+
```

---

## 2. Domain Adapter Design: HomeAssistantSource

### 2.1 Core Traits (Existing - No Changes)

```rust
// core/src/traits.rs - Already exists
#[async_trait]
pub trait Source: Send + Sync {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}

pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,     // entity_id for HA
    pub value: f64,              // 1.0 = open/on, 0.0 = closed/off
    pub tags: HashMap<String, String>,
}
```

### 2.2 New Source Type: Home Assistant WebSocket

```rust
// core/src/sources/home_assistant.rs (NEW FILE)

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::error::{CoreError, CoreResult};
use crate::parsers::Parser;
use crate::traits::{HealthStatus, Source, TimeSeriesPoint};

/// Home Assistant connection configuration
#[derive(Debug, Clone)]
pub struct HomeAssistantConfig {
    /// WebSocket URL (e.g., "ws://homeassistant.local:8123/api/websocket")
    pub websocket_url: String,

    /// Long-lived access token for authentication
    pub access_token: String,

    /// Entity IDs to subscribe to (e.g., ["binary_sensor.window_*"])
    pub entity_filters: Vec<String>,

    /// Reconnect delay with exponential backoff
    pub reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,

    /// Buffer capacity for internal channel
    pub buffer_capacity: usize,

    /// Domain filter (e.g., ["binary_sensor", "sensor", "input_boolean"])
    pub domain_filters: Vec<String>,
}

impl Default for HomeAssistantConfig {
    fn default() -> Self {
        Self {
            websocket_url: "ws://homeassistant.local:8123/api/websocket".to_string(),
            access_token: String::new(),  // Must be provided via env var
            entity_filters: vec!["binary_sensor.window_*".to_string()],
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(60),
            buffer_capacity: 1000,
            domain_filters: vec!["binary_sensor".to_string()],
        }
    }
}

/// Home Assistant WebSocket message types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum HaMessage {
    #[serde(rename = "auth_required")]
    AuthRequired { ha_version: String },

    #[serde(rename = "auth_ok")]
    AuthOk { ha_version: String },

    #[serde(rename = "auth_invalid")]
    AuthInvalid { message: String },

    #[serde(rename = "result")]
    Result { id: u64, success: bool, result: Option<Value> },

    #[serde(rename = "event")]
    Event { id: u64, event: HaEventData },
}

#[derive(Debug, Deserialize)]
struct HaEventData {
    event_type: String,
    data: HaStateChange,
}

#[derive(Debug, Deserialize)]
struct HaStateChange {
    entity_id: String,
    new_state: Option<HaState>,
    old_state: Option<HaState>,
}

#[derive(Debug, Deserialize)]
struct HaState {
    entity_id: String,
    state: String,
    attributes: HashMap<String, Value>,
    last_changed: DateTime<Utc>,
    last_updated: DateTime<Utc>,
}

/// Home Assistant data source using WebSocket API
pub struct HomeAssistantSource {
    config: HomeAssistantConfig,
    parser: Arc<dyn Parser + Send + Sync>,
    cached_points: Arc<Mutex<Vec<TimeSeriesPoint>>>,
    is_running: Arc<Mutex<bool>>,
    connection_healthy: Arc<Mutex<bool>>,
    message_id: Arc<Mutex<u64>>,
}

impl HomeAssistantSource {
    /// Create a new Home Assistant source with injected parser
    pub fn new(
        config: HomeAssistantConfig,
        parser: Box<dyn Parser + Send + Sync>,
    ) -> Self {
        Self {
            config,
            parser: Arc::from(parser),
            cached_points: Arc::new(Mutex::new(Vec::new())),
            is_running: Arc::new(Mutex::new(false)),
            connection_healthy: Arc::new(Mutex::new(false)),
            message_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Convert HA state to TimeSeriesPoint
    fn state_to_point(&self, state: &HaState) -> Option<TimeSeriesPoint> {
        // Convert binary states to numeric values
        let value = match state.state.to_lowercase().as_str() {
            "on" | "open" | "true" | "home" | "detected" => 1.0,
            "off" | "closed" | "false" | "away" | "clear" => 0.0,
            // For numeric sensors, parse the value
            other => other.parse::<f64>().ok()?,
        };

        let mut tags = HashMap::new();
        tags.insert("stream_id".to_string(), "home-events".to_string());
        tags.insert("entity_id".to_string(), state.entity_id.clone());
        tags.insert("state_string".to_string(), state.state.clone());

        // Extract domain and object_id from entity_id
        if let Some((domain, object_id)) = state.entity_id.split_once('.') {
            tags.insert("domain".to_string(), domain.to_string());
            tags.insert("object_id".to_string(), object_id.to_string());
            tags.insert("metric".to_string(), format!("{}_{}", domain, object_id));
        }

        // Add relevant attributes as tags
        if let Some(friendly_name) = state.attributes.get("friendly_name") {
            if let Some(name) = friendly_name.as_str() {
                tags.insert("friendly_name".to_string(), name.to_string());
            }
        }

        if let Some(device_class) = state.attributes.get("device_class") {
            if let Some(class) = device_class.as_str() {
                tags.insert("device_class".to_string(), class.to_string());
            }
        }

        Some(TimeSeriesPoint {
            timestamp: state.last_changed,
            location_id: state.entity_id.clone(),
            value,
            tags,
        })
    }

    /// Start WebSocket connection and event processing
    pub async fn start(&mut self) -> CoreResult<()> {
        info!("Starting Home Assistant source");
        *self.is_running.lock().await = true;

        // Clone for background task
        let config = self.config.clone();
        let cached_points = self.cached_points.clone();
        let is_running = self.is_running.clone();
        let connection_healthy = self.connection_healthy.clone();
        let message_id = self.message_id.clone();

        tokio::spawn(async move {
            Self::connection_loop(
                config,
                cached_points,
                is_running,
                connection_healthy,
                message_id,
            ).await;
        });

        Ok(())
    }

    async fn connection_loop(
        config: HomeAssistantConfig,
        cached_points: Arc<Mutex<Vec<TimeSeriesPoint>>>,
        is_running: Arc<Mutex<bool>>,
        connection_healthy: Arc<Mutex<bool>>,
        message_id: Arc<Mutex<u64>>,
    ) {
        let mut reconnect_attempt = 0u32;

        while *is_running.lock().await {
            match Self::connect_and_process(
                &config,
                &cached_points,
                &is_running,
                &connection_healthy,
                &message_id,
            ).await {
                Ok(_) => {
                    reconnect_attempt = 0;
                }
                Err(e) => {
                    error!("Home Assistant connection error: {}", e);
                    *connection_healthy.lock().await = false;

                    let delay = std::cmp::min(
                        config.reconnect_delay.as_secs() * 2u64.pow(reconnect_attempt),
                        config.max_reconnect_delay.as_secs(),
                    );

                    warn!("Reconnecting in {} seconds (attempt {})", delay, reconnect_attempt);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    reconnect_attempt += 1;
                }
            }
        }
    }

    async fn connect_and_process(
        config: &HomeAssistantConfig,
        cached_points: &Arc<Mutex<Vec<TimeSeriesPoint>>>,
        is_running: &Arc<Mutex<bool>>,
        connection_healthy: &Arc<Mutex<bool>>,
        message_id: &Arc<Mutex<u64>>,
    ) -> CoreResult<()> {
        let (ws_stream, _) = connect_async(&config.websocket_url)
            .await
            .map_err(|e| CoreError::Source(format!("WebSocket connection failed: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // Process messages
        while let Some(msg) = read.next().await {
            if !*is_running.lock().await {
                break;
            }

            let msg = msg.map_err(|e| CoreError::Source(format!("WebSocket error: {}", e)))?;

            if let Message::Text(text) = msg {
                let ha_msg: HaMessage = serde_json::from_str(&text)
                    .map_err(|e| CoreError::Source(format!("JSON parse error: {}", e)))?;

                match ha_msg {
                    HaMessage::AuthRequired { .. } => {
                        // Send auth message
                        let auth = serde_json::json!({
                            "type": "auth",
                            "access_token": config.access_token
                        });
                        write.send(Message::Text(auth.to_string())).await
                            .map_err(|e| CoreError::Source(format!("Send error: {}", e)))?;
                    }
                    HaMessage::AuthOk { .. } => {
                        info!("Authenticated with Home Assistant");
                        *connection_healthy.lock().await = true;

                        // Subscribe to state changes
                        let mut id = message_id.lock().await;
                        let subscribe = serde_json::json!({
                            "id": *id,
                            "type": "subscribe_events",
                            "event_type": "state_changed"
                        });
                        *id += 1;
                        write.send(Message::Text(subscribe.to_string())).await
                            .map_err(|e| CoreError::Source(format!("Send error: {}", e)))?;
                    }
                    HaMessage::AuthInvalid { message } => {
                        return Err(CoreError::Source(format!("Auth failed: {}", message)));
                    }
                    HaMessage::Event { event, .. } => {
                        if event.event_type == "state_changed" {
                            if let Some(new_state) = event.data.new_state {
                                // Check entity filter
                                let entity_id = &new_state.entity_id;
                                let matches_filter = config.entity_filters.iter().any(|f| {
                                    if f.contains('*') {
                                        let pattern = f.replace("*", "");
                                        entity_id.starts_with(&pattern)
                                    } else {
                                        entity_id == f
                                    }
                                });

                                if matches_filter || config.entity_filters.is_empty() {
                                    // Convert to TimeSeriesPoint placeholder
                                    // Real implementation would use self.state_to_point()
                                    let value = match new_state.state.to_lowercase().as_str() {
                                        "on" | "open" | "true" => 1.0,
                                        _ => 0.0,
                                    };

                                    let mut tags = HashMap::new();
                                    tags.insert("stream_id".to_string(), "home-events".to_string());
                                    tags.insert("entity_id".to_string(), entity_id.clone());
                                    tags.insert("metric".to_string(), entity_id.replace('.', "_"));
                                    tags.insert("state_string".to_string(), new_state.state.clone());

                                    let point = TimeSeriesPoint {
                                        timestamp: new_state.last_changed,
                                        location_id: entity_id.clone(),
                                        value,
                                        tags,
                                    };

                                    cached_points.lock().await.push(point);
                                    debug!("Captured state change: {} -> {}", entity_id, new_state.state);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Stop the source
    pub async fn stop(&mut self) -> CoreResult<()> {
        info!("Stopping Home Assistant source");
        *self.is_running.lock().await = false;
        *self.connection_healthy.lock().await = false;
        Ok(())
    }
}

#[async_trait]
impl Source for HomeAssistantSource {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>> {
        let mut cache = self.cached_points.lock().await;
        let points = cache.drain(..).collect();
        Ok(points)
    }

    async fn health_check(&self) -> CoreResult<HealthStatus> {
        let is_healthy = *self.connection_healthy.lock().await;
        let is_running = *self.is_running.lock().await;

        let mut details = HashMap::new();
        details.insert("source_type".to_string(), "home_assistant".to_string());
        details.insert("is_running".to_string(), is_running.to_string());
        details.insert("is_connected".to_string(), is_healthy.to_string());

        Ok(HealthStatus {
            healthy: is_running && is_healthy,
            message: if is_running && is_healthy {
                "Home Assistant connection healthy".to_string()
            } else if is_running {
                "Home Assistant connection unhealthy".to_string()
            } else {
                "Home Assistant source not running".to_string()
            },
            details,
        })
    }
}
```

### 2.3 Source Type Extension

```rust
// Update core/src/types/stream_config.rs

/// Source type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
    HomeAssistant,  // NEW
}
```

---

## 3. Stream Configuration

### 3.1 Stream Definition

```yaml
# config/base/streams/home-events/config.yaml

stream_id: home-events
description: Home automation events from Home Assistant (windows, doors, motion)
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: state_value
    type: float
    nullable: false
    description: "Numeric state (1.0=on/open, 0.0=off/closed)"
  - name: state_string
    type: string
    nullable: true
    description: "Original state string from HA"
  - name: entity_id
    type: string
    nullable: false
    description: "Home Assistant entity ID"
  - name: domain
    type: string
    nullable: false
    description: "HA domain (binary_sensor, sensor, etc.)"
  - name: device_class
    type: string
    nullable: true
    description: "Device class (window, door, motion, etc.)"
  - name: friendly_name
    type: string
    nullable: true
    description: "Human-readable name from HA"
  - name: attributes_json
    type: json
    nullable: true
    description: "Full HA state attributes"

sources:
  - type: home_assistant
    enabled: true
    websocket_url: "${HASS_WEBSOCKET_URL}"
    access_token: "${HASS_ACCESS_TOKEN}"
    entity_filters:
      - "binary_sensor.window_*"
      - "binary_sensor.door_*"
      - "binary_sensor.motion_*"
      - "input_boolean.windows_*"
    domain_filters:
      - "binary_sensor"
      - "input_boolean"
    reconnect_delay_secs: 5
    max_reconnect_delay_secs: 300
    buffer_capacity: 1000
    parser:
      parser_type: home_assistant
      default_tags:
        source: home_assistant
        stream_id: home-events

storage:
  batch_size: 50
  batch_timeout_secs: 30
  buffer_capacity: 500
```

---

## 4. Data Flow Diagram

```
+-------------------+
| Home Assistant    |
| (WebSocket API)   |
+--------+----------+
         |
         | state_changed events
         v
+-------------------+     +-------------------+
| HomeAssistant     |     | IngestionCoord    |
| Source            +---->| (owns channel)    |
| (WebSocket conn)  |     |                   |
+-------------------+     +--------+----------+
                                   |
                                   | TimeSeriesPoint
                                   v
                          +-------------------+
                          | StreamRouter      |
                          | (adds stream_id)  |
                          +--------+----------+
                                   |
                                   v
+----------------------------------------------------------+
|                      Bronze Layer                         |
|  +---------------------------------------------------+   |
|  | ParquetStore                                      |   |
|  | data/home-events/year=YYYY/month=MM/day=DD/      |   |
|  |   readings.parquet                                |   |
|  +---------------------------------------------------+   |
+----------------------------------------------------------+
                                   |
                                   | ETL (future dp-XXX)
                                   v
+----------------------------------------------------------+
|                      Silver Layer                         |
|  +---------------------------------------------------+   |
|  | TimescaleDB                                       |   |
|  | - home_events table (hypertable)                  |   |
|  | - state_changes continuous aggregate              |   |
|  | - window_open_duration materialized view          |   |
|  +---------------------------------------------------+   |
+----------------------------------------------------------+
                                   |
                                   v
+----------------------------------------------------------+
|                      Gold Layer (Features)                |
|  - window_open_ratio_1h                                  |
|  - windows_open_when_ac_on (joined with air-quality)     |
|  - ventilation_score                                     |
+----------------------------------------------------------+
```

---

## 5. Bronze Layer Schema (Parquet)

### 5.1 File Layout

```
data/
  home-events/
    year=2024/
      month=12/
        day=29/
          readings.parquet
```

### 5.2 Parquet Schema

| Column | Type | Description |
|--------|------|-------------|
| timestamp | INT64 (microseconds) | Event timestamp |
| location_id | STRING | Entity ID (e.g., "binary_sensor.window_office") |
| metric | STRING | Derived from entity_id (e.g., "binary_sensor_window_office") |
| value | FLOAT64 | 1.0 = open/on, 0.0 = closed/off |

### 5.3 Tags Storage (In TimeSeriesPoint)

| Tag Key | Example Value |
|---------|---------------|
| stream_id | "home-events" |
| entity_id | "binary_sensor.window_office" |
| domain | "binary_sensor" |
| object_id | "window_office" |
| device_class | "window" |
| friendly_name | "Office Window" |
| state_string | "open" |

---

## 6. Silver Layer Schema (TimescaleDB)

### 6.1 Main Table

```sql
-- Silver layer: Structured home events
CREATE TABLE home_events (
    time TIMESTAMPTZ NOT NULL,
    entity_id TEXT NOT NULL,
    domain TEXT NOT NULL,
    object_id TEXT NOT NULL,
    state_value DOUBLE PRECISION NOT NULL,
    state_string TEXT,
    device_class TEXT,
    friendly_name TEXT,
    attributes JSONB,
    CONSTRAINT home_events_pkey PRIMARY KEY (time, entity_id)
);

-- Convert to hypertable
SELECT create_hypertable('home_events', 'time');

-- Indexes for common queries
CREATE INDEX idx_home_events_entity ON home_events (entity_id, time DESC);
CREATE INDEX idx_home_events_domain ON home_events (domain, time DESC);
CREATE INDEX idx_home_events_device_class ON home_events (device_class, time DESC);
```

### 6.2 Continuous Aggregates

```sql
-- Window open duration per hour
CREATE MATERIALIZED VIEW window_open_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    entity_id,
    friendly_name,
    AVG(state_value) AS open_ratio,  -- 1.0 = always open, 0 = always closed
    COUNT(*) AS state_changes,
    SUM(CASE WHEN state_value = 1 THEN 1 ELSE 0 END) AS open_count
FROM home_events
WHERE domain = 'binary_sensor'
  AND device_class IN ('window', 'door')
GROUP BY time_bucket('1 hour', time), entity_id, friendly_name
WITH NO DATA;

-- Refresh policy
SELECT add_continuous_aggregate_policy('window_open_hourly',
    start_offset => INTERVAL '1 day',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '15 minutes');
```

### 6.3 Feature Views (Gold Layer Prep)

```sql
-- Combined indoor air quality with window state
CREATE VIEW indoor_aq_with_windows AS
SELECT
    a.time,
    a.location_id AS sensor_id,
    a.metric,
    a.value AS aq_value,
    w.open_ratio AS windows_open_ratio,
    w.state_changes AS window_changes
FROM air_quality_silver a
LEFT JOIN window_open_hourly w
    ON time_bucket('1 hour', a.time) = w.bucket
WHERE a.metric IN ('pm25', 'co2', 'temperature', 'humidity');
```

---

## 7. Integration Points

### 7.1 With Existing IngestionCoordinator

The HomeAssistantSource integrates via the existing SourceManager:

```rust
// In apps/air-quality-app/src/coordinator/source_manager.rs
// Add new match arm for HomeAssistant source type

match source_config.source_type {
    SourceType::Mqtt => { /* existing */ },
    SourceType::HttpPoll => { /* existing */ },
    SourceType::HomeAssistant => {
        let ha_config = HomeAssistantConfig {
            websocket_url: get_param(&source_config, "websocket_url")?,
            access_token: std::env::var("HASS_ACCESS_TOKEN")
                .map_err(|_| CoreError::Config("HASS_ACCESS_TOKEN not set".into()))?,
            entity_filters: get_param_vec(&source_config, "entity_filters")?,
            // ... other params
        };

        let parser = create_parser_from_config(&source_config)?;
        let mut source = HomeAssistantSource::new(ha_config, parser);
        source.start().await?;
        // ...
    }
}
```

### 7.2 With StreamRouter

No changes needed - StreamRouter already adds `stream_id` tag based on configuration.

### 7.3 With ParquetStore

No changes needed - ParquetStore already uses `stream_id` tag for partitioning.

---

## 8. Event-Based vs State-Based Design Decision

### 8.1 Recommendation: Hybrid Approach

**Store events, derive state when needed.**

| Aspect | Event-Based | State-Based |
|--------|-------------|-------------|
| Storage | Each state change | Periodic snapshots |
| Query Complexity | Need window functions | Direct lookup |
| Data Volume | Lower (sparse) | Higher (redundant) |
| Missing Data | No gap = same state | Need interpolation |

**Implementation:**
1. **Bronze**: Store raw state_changed events (event-based)
2. **Silver**: Create state reconstruction views (state derived from events)
3. **Gold**: Feature engineering uses continuous aggregates

### 8.2 State Reconstruction Query

```sql
-- Get window state at any point in time
WITH state_at_time AS (
    SELECT
        entity_id,
        state_value,
        time,
        LEAD(time) OVER (PARTITION BY entity_id ORDER BY time) AS next_change
    FROM home_events
    WHERE device_class = 'window'
)
SELECT entity_id, state_value
FROM state_at_time
WHERE $1 >= time AND ($1 < next_change OR next_change IS NULL);
```

---

## 9. Implementation Phases

### Phase 1: Bronze Integration (AIR-008)
- [ ] Implement `HomeAssistantSource` struct
- [ ] Add `SourceType::HomeAssistant` enum variant
- [ ] Create `home-events` stream configuration
- [ ] Integrate with SourceManager
- [ ] Write integration tests with mock HA server

### Phase 2: Silver Layer (dp-002)
- [ ] Create TimescaleDB schema
- [ ] Build Bronze -> Silver ETL
- [ ] Implement continuous aggregates
- [ ] Add data quality checks

### Phase 3: Feature Integration (fe-001)
- [ ] Join window state with air quality data
- [ ] Create ventilation score features
- [ ] Build ML-ready feature views

---

## 10. Dependencies

### Rust Crates
```toml
# Cargo.toml additions
tokio-tungstenite = "0.21"  # WebSocket client
futures-util = "0.3"         # Stream utilities
```

### Environment Variables
```bash
# Required configuration
HASS_WEBSOCKET_URL=ws://homeassistant.local:8123/api/websocket
HASS_ACCESS_TOKEN=<long-lived-access-token>
```

---

## 11. Testing Strategy

### 11.1 Unit Tests
- State conversion logic (on/off to 1.0/0.0)
- Entity filter matching
- Tag extraction from HA state

### 11.2 Integration Tests
- Mock WebSocket server for HA protocol
- End-to-end event capture
- Reconnection with backoff

### 11.3 Manual Testing
- Connect to real Home Assistant instance
- Verify event capture in Parquet files
- Query Bronze layer data

---

## 12. Security Considerations

1. **Token Storage**: Use environment variables or secrets manager, never in config files
2. **Network**: Home Assistant should be on trusted network (local or VPN)
3. **Data Sensitivity**: Window/door state reveals occupancy patterns - consider retention policies
4. **Access Control**: Long-lived token should have minimal required permissions

---

## 13. Future Extensions

### 13.1 Log Stream Support (Mentioned in Scope)
The same pattern can extend to log streams:
- Create `LogStreamSource` implementing `Source` trait
- Use `stream_id: "system-logs"` for partitioning
- Store structured log entries with severity, source, message

### 13.2 Bidirectional Control
Future feature: Send commands back to Home Assistant
```rust
pub trait Actuator: Send + Sync {
    async fn set_state(&self, entity_id: &str, state: &str) -> CoreResult<()>;
}
```

---

## 14. References

- [Home Assistant WebSocket API](https://developers.home-assistant.io/docs/api/websocket)
- [Home Assistant Data Portal](https://data.home-assistant.io)
- NDP ADR-001: Channel Ownership Pattern
- NDP Stream Configuration Specification

---

*Document Version: 1.0.0*
*Last Updated: 2024-12-29*
*Author: ndp-rust-dev*
