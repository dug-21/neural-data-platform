# Pseudocode: Source Adapter Changes

## Overview

This document describes how sources must be updated to emit `RawDataPoint` instead of `Vec<TimeSeriesPoint>`. The key change is that sources no longer parse data - they simply capture the raw payload with minimal metadata extraction.

## Related ADR

- [ADR-001: Bronze Layer Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)

---

## Current vs New Approach

### Current Flow (Parsed at Source)

```
HTTP Response -> Parser -> Vec<TimeSeriesPoint> -> Storage
```

### New Flow (Raw at Source, Parse Later)

```
HTTP Response -> RawDataPoint (with raw_payload) -> Bronze Storage
Bronze Storage -> Silver ETL -> TimeSeriesPoint -> Silver Storage
```

---

## New RawSource Trait

```pseudocode
// New trait for raw data sources (alongside existing Source trait)
TRAIT RawSource: Send + Sync:
    // Fetch raw data points from the source
    ASYNC FUNCTION fetch_raw(self) -> CoreResult<Vec<RawDataPoint>>

    // Health check (same as existing)
    ASYNC FUNCTION health_check(self) -> CoreResult<HealthStatus>

    // Source identifier
    FUNCTION source_id(self) -> String
END TRAIT
```

---

## HTTP Polling Source Changes

### Current Implementation (core/src/sources/http_poll.rs)

```pseudocode
// Current: Returns parsed TimeSeriesPoints
ASYNC FUNCTION poll_sensor(self, sensor: SensorConfig) -> Result<Vec<TimeSeriesPoint>>:
    response = TRY self.client.get(sensor.url).send()
    body = TRY response.text()
    json = TRY serde_json::from_str(body)
    parse_context = ParseContext::new(self.ndp_id, self.context)

    // Currently: Parser extracts metrics
    points = TRY self.parser.parse_with_context(json, timestamp, parse_context)
    RETURN Ok(points)
END FUNCTION
```

### New Implementation

```pseudocode
// New: Returns raw data point with untransformed payload
ASYNC FUNCTION poll_sensor_raw(
    self,
    sensor: SensorConfig
) -> Result<RawDataPoint>:

    response = TRY self.client.get(sensor.url).send()

    IF NOT response.status().is_success():
        RETURN Error("HTTP {status}: {body}")
    END IF

    // Get raw body
    body = TRY response.text()

    // Parse just enough to validate it's JSON
    raw_payload: serde_json::Value = TRY serde_json::from_str(body)
    IF raw_payload IS Error:
        RETURN Error("Invalid JSON response: {error}")
    END IF

    // Create RawDataPoint - NO metric parsing!
    raw_point = RawDataPoint::with_context(
        source_id: self.source_id(),    // "air-quality-Http"
        raw_payload: raw_payload,
        ndp_id: self.ndp_id.clone(),
        context: self.context.clone(),
    )

    RETURN Ok(raw_point)
END FUNCTION
```

---

## Minimal Metadata Extraction

Parsers are simplified to extract only routing metadata, not metric values.

### Current Parser Role (Full Parsing)

```pseudocode
// Current: Extract all metrics
FUNCTION Parser::parse(json) -> Vec<TimeSeriesPoint>:
    points = []
    FOR field IN json.fields():
        IF is_numeric(field.value):
            point = TimeSeriesPoint {
                timestamp: now,
                location_id: json["serialno"],
                value: field.value,
                tags: {"metric": field.name},
                ndp_id: context.ndp_id,
                context: context.context,
            }
            points.push(point)
        END IF
    END FOR
    RETURN points
END FUNCTION
```

### New Metadata Extractor (Minimal)

```pseudocode
// New: Extract only routing metadata for partitioning
STRUCT MetadataExtractor:
    location_id_field: String       // Field name to extract as device ID
    required_fields: Vec<String>    // Fields that must exist for valid payload
END STRUCT

FUNCTION MetadataExtractor::extract(
    self,
    raw_payload: JSON
) -> Result<ExtractedMetadata>:

    // Extract device/location identifier for partitioning
    location_id = raw_payload.get(self.location_id_field)
        OR RETURN Error("Missing required field: {location_id_field}")

    // Validate required fields exist
    FOR field IN self.required_fields:
        IF NOT raw_payload.contains_key(field):
            RETURN Error("Missing required field: {field}")
        END IF
    END FOR

    RETURN Ok(ExtractedMetadata {
        location_id: location_id.to_string(),
        is_valid: true,
    })
END FUNCTION
```

---

## Source Factory Updates

```pseudocode
FUNCTION SourceFactory::create_raw_source(
    config: StreamConfig
) -> Result<Box<dyn RawSource>>:

    source_type = config.source.source_type

    MATCH source_type:
        SourceType::Http => {
            http_config = HttpPollingConfig::from(config)
            source = RawHttpPollingSource::with_context(
                config: http_config,
                ndp_id: config.metadata.ndp_id,
                context: config.metadata.context,
            )?
            RETURN Ok(Box::new(source))
        }

        SourceType::Mqtt => {
            mqtt_config = MqttConfig::from(config)
            source = RawMqttSource::with_context(
                config: mqtt_config,
                ndp_id: config.metadata.ndp_id,
                context: config.metadata.context,
            )?
            RETURN Ok(Box::new(source))
        }

        _ => RETURN Error("Unsupported source type: {source_type}")
    END MATCH
END FUNCTION
```

---

## HTTP Polling Source Refactored

```pseudocode
STRUCT RawHttpPollingSource:
    config: HttpPollingConfig
    client: reqwest::Client
    receiver: Mutex<mpsc::Receiver<RawDataPoint>>
    sender: mpsc::Sender<RawDataPoint>
    is_running: Mutex<bool>
    last_successful_poll: Mutex<HashMap<String, DateTime<Utc>>>
    ndp_id: Option<String>
    context: Option<serde_json::Value>
END STRUCT

IMPLEMENT RawSource FOR RawHttpPollingSource:
    ASYNC FUNCTION fetch_raw(self) -> Result<Vec<RawDataPoint>>:
        receiver = TRY self.receiver.lock()
        points = Vec::new()

        // Drain all available points from the channel
        WHILE let Ok(point) = receiver.try_recv():
            points.push(point)
        END WHILE

        RETURN Ok(points)
    END FUNCTION

    ASYNC FUNCTION health_check(self) -> Result<HealthStatus>:
        // Same as existing implementation
        ...
    END FUNCTION

    FUNCTION source_id(self) -> String:
        // Construct from stream config
        // Format: "{stream_id}-{SourceType}"
        RETURN format!("{}-Http", self.config.stream_id)
    END FUNCTION
END IMPLEMENT

IMPLEMENT RawHttpPollingSource:
    ASYNC FUNCTION poll_all_sensors_raw(self) -> Result<()>:
        FOR sensor IN self.config.sensors:
            MATCH self.poll_sensor_raw(sensor).await:
                Ok(raw_point) => {
                    // Update last successful poll time
                    last_poll = TRY self.last_successful_poll.lock()
                    last_poll.insert(sensor.serial_number, Utc::now())

                    // Send to channel
                    IF let Err(e) = self.sender.send(raw_point).await:
                        log::warn!("Failed to send raw point: {}", e)
                    END IF
                }
                Err(e) => {
                    log::error!("Failed to poll sensor {}: {}", sensor.serial_number, e)
                }
            END MATCH
        END FOR

        RETURN Ok(())
    END FUNCTION

    ASYNC FUNCTION start(self) -> Result<()>:
        log::info!("Starting raw HTTP polling source")

        IF self.config.sensors.is_empty():
            RETURN Error("No sensors configured")
        END IF

        *self.is_running.lock() = true

        // Clone for background task
        source_clone = self.clone()

        tokio::spawn(async move {
            IF let Err(e) = source_clone.polling_loop_raw():
                log::error!("Raw HTTP polling loop failed: {}", e)
            END IF
        })

        // Initial poll
        self.poll_all_sensors_raw().await?

        RETURN Ok(())
    END FUNCTION
END IMPLEMENT
```

---

## Ingestion Coordinator Changes

```pseudocode
// Updated to use RawSource and RawDataPoint
STRUCT IngestionCoordinator:
    raw_source: Box<dyn RawSource>
    raw_store: Box<dyn RawStore>
    tx: mpsc::Sender<RawDataPoint>
    rx: mpsc::Receiver<RawDataPoint>
    is_running: bool
END STRUCT

IMPLEMENT IngestionCoordinator:
    ASYNC FUNCTION run(self) -> Result<()>:
        self.is_running = true

        // Background task to collect from source
        SPAWN {
            WHILE self.is_running:
                MATCH self.raw_source.fetch_raw().await:
                    Ok(points) => {
                        FOR point IN points:
                            IF let Err(e) = self.tx.send(point).await:
                                log::warn!("Channel send failed: {}", e)
                            END IF
                        END FOR
                    }
                    Err(e) => log::error!("Source fetch failed: {}", e)
                END MATCH

                tokio::time::sleep(Duration::from_millis(100)).await
            END WHILE
        }

        // Main processing loop
        batch: Vec<RawDataPoint> = Vec::new()
        batch_timeout = Duration::from_secs(5)
        batch_size = 100

        WHILE let Some(point) = tokio::select! {
            point = self.rx.recv() => point,
            _ = tokio::time::sleep(batch_timeout) => None,
        }:
            batch.push(point)

            IF batch.len() >= batch_size:
                TRY self.raw_store.write_raw_batch(batch.drain(..).collect()).await
            END IF
        END WHILE

        // Flush remaining
        IF NOT batch.is_empty():
            TRY self.raw_store.write_raw_batch(batch).await
        END IF

        RETURN Ok(())
    END FUNCTION
END IMPLEMENT
```

---

## Dual Mode Support (Transition Period)

During migration, sources can support both modes:

```pseudocode
ENUM SourceMode:
    Parsed,      // Legacy: emit Vec<TimeSeriesPoint>
    Raw,         // New: emit Vec<RawDataPoint>
    DualWrite,   // Transition: emit both
END ENUM

TRAIT DualModeSource: Source + RawSource:
    // Get current mode
    FUNCTION mode(self) -> SourceMode

    // Set mode
    FUNCTION set_mode(self, mode: SourceMode)
END TRAIT

IMPLEMENT DualModeSource FOR HttpPollingSource:
    ASYNC FUNCTION fetch(self) -> Result<Vec<TimeSeriesPoint>>:
        IF self.mode == SourceMode::Raw:
            // Return empty - use fetch_raw() instead
            RETURN Ok(Vec::new())
        END IF

        // Legacy behavior
        ...
    END FUNCTION

    ASYNC FUNCTION fetch_raw(self) -> Result<Vec<RawDataPoint>>:
        IF self.mode == SourceMode::Parsed:
            // Return empty - use fetch() instead
            RETURN Ok(Vec::new())
        END IF

        // New behavior
        ...
    END FUNCTION
END IMPLEMENT
```

---

## Configuration Changes

```yaml
# config/base/streams/air-quality.yaml
stream:
  id: air-quality
  metadata:
    ndp_id: "air-quality-office-001"
    context:
      room: "office"
      floor: 2
  source:
    type: Http
    # New: output mode
    output_mode: raw  # "raw" | "parsed" | "dual"
    poll_interval_secs: 60
    sensors:
      - serial: "ecda3b2aa820"
        url: "http://airgradient_ecda3b2aa820.local/measures/current"
```

---

## Rust Implementation Signature

```rust
use crate::error::CoreResult;
use crate::traits::{HealthStatus, RawDataPoint};
use async_trait::async_trait;

/// Trait for sources that emit raw data points
#[async_trait]
pub trait RawSource: Send + Sync {
    /// Fetch raw data points from the source
    async fn fetch_raw(&self) -> CoreResult<Vec<RawDataPoint>>;

    /// Health check
    async fn health_check(&self) -> CoreResult<HealthStatus>;

    /// Get source identifier (e.g., "air-quality-Http")
    fn source_id(&self) -> String;
}

/// Raw HTTP polling source
pub struct RawHttpPollingSource {
    config: HttpPollingConfig,
    client: reqwest::Client,
    receiver: Arc<Mutex<mpsc::Receiver<RawDataPoint>>>,
    sender: mpsc::Sender<RawDataPoint>,
    is_running: Arc<Mutex<bool>>,
    last_successful_poll: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
    ndp_id: Option<String>,
    context: Option<serde_json::Value>,
}

impl RawHttpPollingSource {
    pub fn with_context(
        config: HttpPollingConfig,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> CoreResult<Self> {
        // Implementation
        todo!()
    }

    pub async fn start(&mut self) -> CoreResult<()> {
        // Implementation
        todo!()
    }

    pub async fn stop(&mut self) -> CoreResult<()> {
        // Implementation
        todo!()
    }

    async fn poll_sensor_raw(&self, sensor: &SensorConfig) -> CoreResult<RawDataPoint> {
        // Implementation
        todo!()
    }
}

#[async_trait]
impl RawSource for RawHttpPollingSource {
    async fn fetch_raw(&self) -> CoreResult<Vec<RawDataPoint>> {
        // Implementation
        todo!()
    }

    async fn health_check(&self) -> CoreResult<HealthStatus> {
        // Implementation
        todo!()
    }

    fn source_id(&self) -> String {
        format!("{}-Http", self.config.stream_id)
    }
}
```

---

## File Location

**Target**: `core/src/sources/http_poll.rs` (add `RawHttpPollingSource`)

## Related Files

| File | Change |
|------|--------|
| `core/src/traits.rs` | Add `RawSource` trait and `RawDataPoint` |
| `core/src/sources/http_poll.rs` | Add `RawHttpPollingSource` |
| `core/src/sources/mod.rs` | Export new types |
| `core/src/ingestion/mod.rs` | Update `IngestionCoordinator` |
| `config/base/streams/*.yaml` | Add `output_mode` configuration |
