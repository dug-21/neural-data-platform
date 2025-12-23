# How to Add a New Data Source

**Document Type**: Procedure
**Version**: 1.0.0
**Last Updated**: 2025-12-16
**Applies To**: Neural Data Platform v1.x

---

## Overview

This guide explains how to add a new data source type to the Neural Data Platform. A "source" is any system that provides data to be ingested (e.g., MQTT broker, HTTP API, webhook endpoint, file watcher).

### Prerequisites

- Rust development environment
- Understanding of async Rust (tokio)
- Familiarity with the `Source` trait pattern
- Access to the target data source for testing

### Time Estimate

- **Simple Source** (HTTP polling): 2-4 hours
- **Complex Source** (WebSocket, custom protocol): 4-8 hours
- **Testing & Integration**: 2-4 hours

---

## Architecture Context

### Source Trait (Port)

All sources implement the `Source` trait defined in `neural-core`:

```rust
// neural-core/src/traits.rs
#[async_trait]
pub trait Source: Send + Sync {
    /// Fetch available data points
    async fn fetch(&self) -> Result<Vec<TimeSeriesPoint>, CoreError>;

    /// Check if source is healthy and connected
    async fn health_check(&self) -> Result<HealthStatus, CoreError>;
}
```

### Existing Source Implementations

| Source Type | File | Description |
|-------------|------|-------------|
| `MqttSource` | `neural-core/src/sources/mqtt.rs` | Push-based MQTT subscription |
| `HttpPollingSource` | Planned | Poll-based HTTP API calls |
| `WebhookHandler` | Planned | Push-based webhook receiver |

---

## Step-by-Step Procedure

### Step 1: Add Source Type Enum Variant

**File**: `core/src/types/stream_config.rs`

Add your new source type to the `SourceType` enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
    // Add your new type here:
    YourNewSource,  // e.g., WebSocket, Modbus, etc.
}
```

### Step 2: Create Source Configuration Struct

**File**: `neural-core/src/sources/your_source.rs` (new file)

Define the configuration your source needs:

```rust
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for YourNewSource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YourSourceConfig {
    /// Endpoint URL or address
    pub endpoint: String,

    /// Polling interval (for poll-based sources)
    #[serde(default = "default_interval")]
    pub interval: Duration,

    /// Connection timeout
    #[serde(default = "default_timeout")]
    pub timeout: Duration,

    /// Optional authentication
    pub auth: Option<AuthConfig>,

    /// Buffer capacity for internal channel
    #[serde(default = "default_buffer")]
    pub buffer_capacity: usize,
}

fn default_interval() -> Duration {
    Duration::from_secs(60)
}

fn default_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_buffer() -> usize {
    1000
}

impl Default for YourSourceConfig {
    fn default() -> Self {
        Self {
            endpoint: "localhost".to_string(),
            interval: default_interval(),
            timeout: default_timeout(),
            auth: None,
            buffer_capacity: default_buffer(),
        }
    }
}
```

### Step 3: Implement the Source Struct

**File**: `neural-core/src/sources/your_source.rs`

```rust
use crate::{CoreError, HealthStatus, TimeSeriesPoint};
use crate::traits::Source;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Your new data source implementation
pub struct YourNewSource {
    config: YourSourceConfig,
    // Internal state (connection handle, buffer, etc.)
    client: Option<YourClient>,
    last_fetch: Arc<RwLock<Option<chrono::DateTime<chrono::Utc>>>>,
}

impl YourNewSource {
    /// Create a new source instance
    pub fn new(config: YourSourceConfig) -> Self {
        info!("Creating YourNewSource with endpoint: {}", config.endpoint);
        Self {
            config,
            client: None,
            last_fetch: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the source (establish connections, subscriptions, etc.)
    pub async fn start(&mut self) -> Result<(), CoreError> {
        info!("Starting YourNewSource...");

        // Initialize your client/connection
        let client = YourClient::connect(&self.config.endpoint, self.config.timeout)
            .await
            .map_err(|e| CoreError::Source(format!("Connection failed: {}", e)))?;

        self.client = Some(client);
        info!("YourNewSource started successfully");
        Ok(())
    }

    /// Stop the source gracefully
    pub async fn stop(&mut self) -> Result<(), CoreError> {
        info!("Stopping YourNewSource...");
        if let Some(client) = self.client.take() {
            client.disconnect().await?;
        }
        Ok(())
    }

    /// Transform raw data into TimeSeriesPoint
    fn transform_data(&self, raw: RawData) -> Result<TimeSeriesPoint, CoreError> {
        // Convert your source's data format to TimeSeriesPoint
        let mut fields = std::collections::HashMap::new();
        fields.insert("value".to_string(), raw.value);

        let mut tags = std::collections::HashMap::new();
        tags.insert("source".to_string(), "your-source".to_string());
        tags.insert("sensor_id".to_string(), raw.sensor_id);

        Ok(TimeSeriesPoint {
            timestamp: raw.timestamp,
            fields,
            tags,
        })
    }
}

#[async_trait]
impl Source for YourNewSource {
    async fn fetch(&self) -> Result<Vec<TimeSeriesPoint>, CoreError> {
        let client = self.client.as_ref()
            .ok_or_else(|| CoreError::Source("Source not started".to_string()))?;

        // Fetch data from your source
        let raw_data = client.poll()
            .await
            .map_err(|e| CoreError::Source(format!("Fetch failed: {}", e)))?;

        // Transform to TimeSeriesPoint
        let points: Vec<TimeSeriesPoint> = raw_data
            .into_iter()
            .filter_map(|raw| {
                match self.transform_data(raw) {
                    Ok(point) => Some(point),
                    Err(e) => {
                        warn!("Failed to transform data: {}", e);
                        None
                    }
                }
            })
            .collect();

        // Update last fetch time
        let mut last = self.last_fetch.write().await;
        *last = Some(chrono::Utc::now());

        debug!("Fetched {} points from YourNewSource", points.len());
        Ok(points)
    }

    async fn health_check(&self) -> Result<HealthStatus, CoreError> {
        let healthy = self.client.is_some();
        let mut details = std::collections::HashMap::new();

        if let Some(ref client) = self.client {
            details.insert(
                "connected".to_string(),
                serde_json::json!(client.is_connected()),
            );
        }

        if let Some(last) = *self.last_fetch.read().await {
            details.insert(
                "last_fetch".to_string(),
                serde_json::json!(last.to_rfc3339()),
            );
        }

        Ok(HealthStatus {
            healthy,
            message: if healthy {
                "YourNewSource is operational".to_string()
            } else {
                "YourNewSource not connected".to_string()
            },
            details,
        })
    }
}
```

### Step 4: Export from Module

**File**: `neural-core/src/sources/mod.rs`

Add your source to the module exports:

```rust
mod mqtt;
mod your_source;  // Add this line

pub use mqtt::{MqttSource, MqttConfig};
pub use your_source::{YourNewSource, YourSourceConfig};  // Add this line
```

### Step 5: Update lib.rs Exports

**File**: `neural-core/src/lib.rs`

Ensure your source is exported:

```rust
pub mod sources;

pub use sources::{
    MqttSource, MqttConfig,
    YourNewSource, YourSourceConfig,  // Add this line
};
```

### Step 6: Create Source Handler (if using channel pattern)

If your source should run continuously and send data through a channel (like `MqttHandler`):

**File**: `apps/air-quality-app/src/ingestion/your_handler.rs` (new file)

```rust
use neural_core::{CoreError, TimeSeriesPoint, YourNewSource, YourSourceConfig};
use neural_core::traits::{HealthStatus, Source};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Handler that wraps YourNewSource and forwards to channel
pub struct YourHandler {
    source: YourNewSource,
    sender: mpsc::Sender<TimeSeriesPoint>,
}

impl YourHandler {
    pub async fn new(
        config: YourSourceConfig,
        sender: mpsc::Sender<TimeSeriesPoint>,
    ) -> Result<Self, CoreError> {
        info!("Initializing YourHandler");

        let mut source = YourNewSource::new(config);
        source.start().await?;

        Ok(Self { source, sender })
    }

    /// Run the ingestion loop
    pub async fn run(&self) -> Result<(), CoreError> {
        info!("Starting YourHandler ingestion loop");

        loop {
            match self.source.fetch().await {
                Ok(points) => {
                    for point in points {
                        if let Err(e) = self.sender.send(point).await {
                            error!("Failed to send point: {}", e);
                            return Err(CoreError::Source(format!("Channel closed: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    warn!("Fetch error (will retry): {}", e);
                }
            }

            // Wait before next fetch (for poll-based sources)
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }

    pub async fn health_check(&self) -> Result<HealthStatus, CoreError> {
        self.source.health_check().await
    }
}
```

### Step 7: Register in SourceManager (for multi-stream)

**File**: `apps/air-quality-app/src/coordinator/source_manager.rs`

Add handling for your new source type:

```rust
match source_config.source_type {
    SourceType::Mqtt => {
        // existing MQTT handling
    }
    SourceType::HttpPoll => {
        // existing HTTP polling handling
    }
    // Add your new source type:
    SourceType::YourNewSource => {
        let config = parse_your_source_config(&source_config)?;
        let handler = YourHandler::new(config, router_tx.clone()).await?;

        tokio::spawn(async move {
            tokio::select! {
                result = handler.run() => result,
                _ = shutdown_rx => {
                    info!("YourHandler shutting down");
                    Ok(())
                }
            }
        })
    }
    _ => {
        return Err(CoreError::Source(
            format!("Unsupported source type: {:?}", source_config.source_type)
        ));
    }
}
```

### Step 8: Write Tests

**File**: `neural-core/src/sources/your_source.rs` (bottom of file)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = YourSourceConfig::default();
        assert_eq!(config.buffer_capacity, 1000);
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_health_check_before_start() {
        let config = YourSourceConfig::default();
        let source = YourNewSource::new(config);

        let health = source.health_check().await.unwrap();
        assert!(!health.healthy);
        assert!(health.message.contains("not connected"));
    }

    #[tokio::test]
    #[ignore] // Run with --ignored when test server available
    async fn test_fetch_returns_points() {
        let config = YourSourceConfig {
            endpoint: "test-endpoint".to_string(),
            ..Default::default()
        };

        let mut source = YourNewSource::new(config);
        source.start().await.expect("Should start");

        let points = source.fetch().await.expect("Should fetch");
        assert!(!points.is_empty());
    }
}
```

---

## Configuration Example

### Stream Config with Your Source

```yaml
# config/streams/my-stream/sources.yaml
sources:
  - type: your_new_source
    enabled: true
    endpoint: "wss://data.example.com/stream"
    interval: 60  # seconds
    timeout: 30   # seconds
    auth:
      type: bearer
      token_env: MY_SOURCE_TOKEN
```

### Configuration Sync (GitOps)

**IMPORTANT**: Stream and source configurations are now managed via GitOps YAML files and automatically synced to etcd. Manual `etcdctl put` commands are deprecated.

**1. Add your source config to the stream's YAML file:**
```bash
# Edit the stream configuration
vim config/base/streams/my-stream/config.yaml

# Add your source under the 'sources:' array
```

**2. Sync configurations to etcd:**
```bash
# From repository root
cd /workspaces/neural-data-platform

# Sync all configurations
ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production

# Or via deployment script
cd deploy/pi && ./deploy.sh sync
```

**3. Verify source is registered:**
```bash
docker exec etcd etcdctl get --prefix /streams/my-stream/ --keys-only
```

> **Note**: The application's `ConfigSyncService` also syncs configs automatically on startup.

---

## Checklist

Before merging your new source:

- [ ] `SourceType` enum updated in `stream_config.rs`
- [ ] Configuration struct created with sensible defaults
- [ ] `Source` trait implemented correctly
- [ ] Error handling follows existing patterns
- [ ] Logging uses `tracing` macros
- [ ] Health check provides useful diagnostics
- [ ] Handler wraps source for channel pattern (if applicable)
- [ ] SourceManager updated to spawn new type
- [ ] Unit tests written and passing
- [ ] Integration test with real source (marked `#[ignore]`)
- [ ] Documentation updated

---

## Buffer Capacity Sizing

When adding sources, proper buffer sizing prevents silent data loss.

### The Problem

Sources using `array_iterator` parsers can generate many points per poll:
- NWS forecast: 156 periods × 7 metrics = **1092 points per poll**
- At startup, initial poll + background loop may fire simultaneously
- Default `buffer_capacity` of 1000 causes overflow and data loss

### Sizing Formula

```
buffer_capacity = array_length × metrics_count × 2.5
```

### Configuration Example

```yaml
sources:
  - type: http_poll
    buffer_capacity: 2500  # NWS generates ~1092 points per poll
    parser:
      parser_type: array_iterator
      array_config:
        array_path: properties.periods  # 156 elements
        element_mappings:  # 7 metrics per element
          - path: temperature
          - path: dewpoint.value
          # ... more metrics
```

### Source Type Buffer Guidelines

| Source Type | Expected Points | Recommended Buffer |
|-------------|-----------------|-------------------|
| Flat JSON (MQTT) | 1-10 per poll | 100-500 |
| JSON Path (single obs) | 1-20 per poll | 100-500 |
| Array Iterator (small) | 50-200 per poll | 500-1000 |
| Array Iterator (large) | 500-2000+ per poll | 2500+ |

---

## Guiding Principle: Collect All Available Information

**Capture everything the source provides.** Storage is cheap; missing historical data is expensive.

### Why This Matters

- Future analysis without re-polling historical data
- Lead time calculations (forecast issue time vs valid time)
- Data quality monitoring (compare predictions to actuals)
- ML feature engineering flexibility

### Application

**1. Document-Level Metadata**

Extract metadata from the response wrapper:

```yaml
metadata_tags:
  - path: properties.generatedAt
    tag_name: forecast_generated_at
metadata_metrics:
  - path: properties.updateTime
    metric_name: forecast_issue_time
    value_type: timestamp
```

**2. All Available Fields**

Map every meaningful field, even if currently unused:

```yaml
element_mappings:
  - path: temperature
    metric_name: temperature
  - path: dewpoint.value           # Often overlooked
    metric_name: dewpoint
  - path: shortForecast            # Text descriptions have value
    metric_name: short_forecast
```

**3. Schema Documentation**

Define all fields with proper units and ranges:

```yaml
fields:
  - name: forecast_issue_time
    type: float
    nullable: true
    unit: epoch_seconds
    description: "Forecast issue timestamp as epoch seconds"
```

### Anti-Pattern

❌ "We only need temperature, skip the rest"
✅ "Capture everything; filter at query time"

---

## Troubleshooting

### Source Won't Connect

1. Check endpoint configuration
2. Verify network connectivity from container
3. Check authentication credentials
4. Review container logs: `docker logs air-quality-app`

### Data Not Appearing

1. Verify source is returning data: check health endpoint
2. Check channel is not full (increase `buffer_capacity` - see sizing guide above)
3. Verify StorageWriter is running
4. Check Parquet files: `ls -la /app/data/`

### Data Loss / Missing Points

1. Check if source uses `array_iterator` parser
2. Calculate expected points: `array_length × metrics_count`
3. Increase `buffer_capacity` to 2.5x expected points
4. Check logs for "forwarded X points" matching "Polled endpoint - X points"

### Performance Issues

1. Adjust polling interval (not too frequent)
2. Increase batch size in StorageWriter
3. Check memory usage: `docker stats`
4. For array sources, ensure buffer_capacity handles concurrent polls

---

## References

- [MqttSource Implementation](../../neural-core/src/sources/mqtt.rs) - Reference implementation
- [MqttHandler](../../apps/air-quality-app/src/ingestion/mqtt_handler.rs) - Handler pattern
- [Source Trait](../../neural-core/src/traits.rs) - Trait definition
- [COORDINATOR_INTERFACES.md](../../product/features/air-004/architecture/COORDINATOR_INTERFACES.md) - Full interface docs
