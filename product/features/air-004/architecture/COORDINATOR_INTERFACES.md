# AIR-004 Coordinator Module Interfaces

**Document Information**
- Feature ID: AIR-004
- Version: 1.0.0
- Status: Design Phase
- Created: 2025-12-15
- Author: ARCHITECT Agent
- Purpose: Define interfaces for multi-stream ingestion coordinator

---

## Executive Summary

This document defines the interfaces and architectural patterns for the AIR-004 IngestionCoordinator module, which orchestrates multi-stream data ingestion. The design follows the existing hexagonal architecture pattern established in AIR-001/002/003 and preserves backward compatibility.

**Core Design Principles**:
1. Reuse existing patterns (MqttHandler, StorageWriter, ConfigClient)
2. Channel-based async communication (tokio mpsc)
3. Trait-based abstraction for testability (London TDD)
4. Graceful shutdown with proper resource cleanup
5. Health check support for monitoring

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│                   IngestionCoordinator                       │
│  ┌───────────────────────────────────────────────────────┐   │
│  │  Initialization Phase                                 │   │
│  │  1. Load StreamRegistry from etcd                     │   │
│  │  2. Initialize StorageLayerManager                    │   │
│  │  3. Spawn SourceManager                              │   │
│  │  4. Spawn IngestionRouter                            │   │
│  │  5. Create StorageWriter per stream                  │   │
│  └───────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌───────────────────────────────────────────────────────┐   │
│  │  Runtime Phase                                        │   │
│  │  - Watch registry for stream changes                 │   │
│  │  - Route records from sources to storage             │   │
│  │  - Monitor health of all components                  │   │
│  └───────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌───────────────────────────────────────────────────────┐   │
│  │  Shutdown Phase                                       │   │
│  │  1. Stop accepting new data (close sources)          │   │
│  │  2. Drain in-flight records                          │   │
│  │  3. Flush all storage writers                        │   │
│  │  4. Close registry watch                             │   │
│  └───────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘

         ┌──────────────┐
         │ SourceManager│
         └───────┬──────┘
                 │ spawns
        ┌────────┼────────┐
        ▼        ▼        ▼
    MqttSource HttpPoller Webhook
        │        │        │
        └────────┼────────┘
                 ▼
         ┌──────────────┐
         │IngestionRouter│
         └───────┬───────┘
                 │ validates & routes
        ┌────────┼────────┐
        ▼        ▼        ▼
   Writer-1  Writer-2  Writer-3
        │        │        │
        └────────┼────────┘
                 ▼
      StorageLayerManager
      (Bronze + Silver)
```

---

## Module Structure

```
apps/air-quality-app/src/coordinator/
├── mod.rs                      # Module exports
├── ingestion_coordinator.rs    # Main coordinator orchestration
├── source_manager.rs           # Source spawning and lifecycle
├── router.rs                   # Validation and routing logic
└── types.rs                    # Shared coordinator types
```

---

## 1. IngestionCoordinator Interface

### Purpose
Orchestrates all components of the multi-stream ingestion pipeline. Responsible for initialization, runtime coordination, and graceful shutdown.

### Design Pattern
Follows the "main.rs orchestration" pattern established in AIR-002, where main.rs coordinates MqttHandler and StorageWriter via channels.

### Interface

```rust
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock, oneshot};
use tokio::task::JoinHandle;
use neural_core::{CoreError, StreamConfig};
use config_client::StreamRegistry;

/// Configuration for the ingestion coordinator
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// etcd endpoints for stream registry
    pub etcd_endpoints: Vec<String>,

    /// Base path for storage (Bronze layer)
    pub storage_base_path: String,

    /// Enable TimescaleDB (Silver layer)
    pub timescale_enabled: bool,

    /// TimescaleDB connection string (if enabled)
    pub timescale_url: Option<String>,

    /// Default channel buffer capacity
    pub buffer_capacity: usize,

    /// Default batch size for storage writers
    pub batch_size: usize,

    /// Default batch timeout in seconds
    pub batch_timeout_secs: u64,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            etcd_endpoints: vec!["http://localhost:2379".to_string()],
            storage_base_path: "/app/data".to_string(),
            timescale_enabled: false,
            timescale_url: None,
            buffer_capacity: 1000,
            batch_size: 100,
            batch_timeout_secs: 5,
        }
    }
}

/// Main coordinator for multi-stream ingestion
pub struct IngestionCoordinator {
    config: CoordinatorConfig,
    registry: Arc<StreamRegistry>,
    source_manager: Arc<SourceManager>,
    router: Arc<IngestionRouter>,
    layer_manager: Arc<StorageLayerManager>,

    // Tracks active storage writer tasks
    storage_writers: Arc<RwLock<HashMap<String, WriterHandle>>>,

    // Shutdown coordination
    shutdown_tx: Option<oneshot::Sender<()>>,
}

/// Handle for a storage writer task
struct WriterHandle {
    stream_id: String,
    channel_tx: mpsc::Sender<StreamRecord>,
    task_handle: JoinHandle<Result<(), CoreError>>,
}

impl IngestionCoordinator {
    /// Create a new coordinator with the given configuration
    ///
    /// # Errors
    /// Returns error if:
    /// - Cannot connect to etcd
    /// - Cannot initialize storage layer
    /// - Cannot load initial streams from registry
    pub async fn new(config: CoordinatorConfig) -> Result<Self, CoreError>;

    /// Initialize all components without starting the runtime loop
    ///
    /// This loads all stream configurations from the registry and spawns
    /// the necessary sources and storage writers.
    ///
    /// # Errors
    /// Returns error if stream initialization fails
    async fn initialize(&mut self) -> Result<(), CoreError>;

    /// Start the coordinator runtime
    ///
    /// This enters the main event loop that:
    /// - Watches for stream registry changes
    /// - Routes records from sources to storage
    /// - Monitors component health
    /// - Handles graceful shutdown
    ///
    /// This method blocks until a shutdown signal is received.
    ///
    /// # Errors
    /// Returns error if runtime encounters unrecoverable error
    pub async fn run(mut self) -> Result<(), CoreError>;

    /// Request graceful shutdown
    ///
    /// This sends a shutdown signal to the runtime loop.
    /// Call `run()` to block until shutdown completes.
    pub fn shutdown(&mut self);

    /// Spawn a storage writer for a specific stream
    ///
    /// Creates a channel-based pipeline: IngestionRouter -> StorageWriter
    ///
    /// # Errors
    /// Returns error if writer spawning fails
    async fn spawn_storage_writer(
        &self,
        stream_id: String,
        config: &StreamConfig,
    ) -> Result<WriterHandle, CoreError>;

    /// Handle stream registry change event
    ///
    /// - StreamAdded: Spawn new sources and storage writer
    /// - StreamUpdated: Gracefully restart affected sources
    /// - StreamDeleted: Shutdown sources and writer, flush data
    async fn handle_stream_event(
        &mut self,
        event: StreamEvent,
    ) -> Result<(), CoreError>;

    /// Check health of all coordinator components
    ///
    /// Returns health status for:
    /// - StreamRegistry connection
    /// - SourceManager (all sources)
    /// - StorageLayerManager (Bronze + Silver)
    /// - Active storage writers
    pub async fn health_check(&self) -> Result<CoordinatorHealth, CoreError>;
}

/// Health status for the entire coordinator
#[derive(Debug, Clone, serde::Serialize)]
pub struct CoordinatorHealth {
    pub healthy: bool,
    pub registry_connected: bool,
    pub active_streams: usize,
    pub active_sources: usize,
    pub storage_bronze_healthy: bool,
    pub storage_silver_healthy: bool,
    pub writers_healthy: usize,
    pub writers_failed: usize,
}
```

### Implementation Pattern

Following AIR-002 main.rs pattern:

```rust
impl IngestionCoordinator {
    pub async fn new(config: CoordinatorConfig) -> Result<Self, CoreError> {
        // 1. Initialize registry (like loading AppConfig in main.rs)
        let registry = Arc::new(
            StreamRegistry::new(&config.etcd_endpoints.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
            ).await?
        );

        // 2. Initialize storage (like creating ParquetStore in main.rs)
        let bronze_store = Arc::new(MultiStreamStore::new(&config.storage_base_path)?);

        let silver_adapter = if config.timescale_enabled {
            Some(Arc::new(
                TimescaleAdapter::new(
                    config.timescale_url.as_ref()
                        .ok_or(CoreError::Config("TimescaleDB URL required".into()))?
                ).await?
            ))
        } else {
            None
        };

        let layer_manager = Arc::new(
            StorageLayerManager::new(bronze_store, silver_adapter)
        );

        // 3. Initialize router
        let router = Arc::new(IngestionRouter::new(Arc::clone(&registry)));

        // 4. Initialize source manager
        let source_manager = Arc::new(
            SourceManager::new(Arc::clone(&router))
        );

        Ok(Self {
            config,
            registry,
            source_manager,
            router,
            layer_manager,
            storage_writers: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
        })
    }

    pub async fn run(mut self) -> Result<(), CoreError> {
        // Initialize all streams
        self.initialize().await?;

        // Set up shutdown channel (like main.rs)
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // Watch for stream changes
        let registry = Arc::clone(&self.registry);
        let stream_event_rx = registry.watch_streams().await?;

        // Main event loop (like main.rs tokio::select!)
        tokio::select! {
            // Handle stream events
            event = stream_event_rx.recv() => {
                if let Some(event) = event {
                    self.handle_stream_event(event).await?;
                }
            }

            // Handle shutdown signal
            _ = &mut shutdown_rx => {
                tracing::info!("Shutdown signal received");
                self.shutdown_all_components().await?;
            }
        }

        Ok(())
    }

    async fn shutdown_all_components(&mut self) -> Result<(), CoreError> {
        // 1. Stop sources (like dropping sender in main.rs)
        tracing::info!("Shutting down all sources");
        self.source_manager.shutdown_all().await?;

        // 2. Wait for storage writers to complete (like joining task in main.rs)
        tracing::info!("Flushing storage writers");
        let mut writers = self.storage_writers.write().await;

        for (stream_id, handle) in writers.drain() {
            // Close channel to signal shutdown
            drop(handle.channel_tx);

            // Wait for task completion
            match handle.task_handle.await {
                Ok(Ok(_)) => {
                    tracing::info!("Storage writer for {} completed", stream_id);
                }
                Ok(Err(e)) => {
                    tracing::error!("Storage writer for {} failed: {}", stream_id, e);
                }
                Err(e) => {
                    tracing::error!("Failed to join writer task for {}: {}", stream_id, e);
                }
            }
        }

        tracing::info!("Shutdown complete");
        Ok(())
    }
}
```

---

## 2. SourceManager Interface

### Purpose
Manages lifecycle of data sources (MQTT, HTTP, Webhook). Responsible for spawning, health checking, and graceful shutdown of source tasks.

### Design Pattern
Follows the "task handle registry" pattern. Each source runs in an isolated tokio task with a shutdown channel.

### Interface

```rust
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock, oneshot};
use tokio::task::JoinHandle;
use neural_core::{CoreError, SourceConfig, SourceType, StreamRecord};

/// Manages spawning and lifecycle of data sources
pub struct SourceManager {
    router: Arc<IngestionRouter>,
    active_sources: Arc<RwLock<HashMap<String, SourceHandle>>>,
}

/// Handle for an active source task
pub struct SourceHandle {
    pub source_id: String,
    pub stream_id: String,
    pub source_type: SourceType,
    pub task_handle: JoinHandle<Result<(), CoreError>>,
    pub shutdown_tx: oneshot::Sender<()>,
}

impl SourceManager {
    /// Create a new source manager
    ///
    /// # Arguments
    /// * `router` - IngestionRouter for forwarding records
    pub fn new(router: Arc<IngestionRouter>) -> Self;

    /// Spawn a new data source for a stream
    ///
    /// This creates the appropriate source type (MQTT, HTTP, Webhook) based
    /// on the configuration and spawns it in a tokio task.
    ///
    /// # Arguments
    /// * `stream_id` - Stream identifier
    /// * `source_config` - Source-specific configuration
    ///
    /// # Returns
    /// Source ID if successful
    ///
    /// # Errors
    /// Returns error if:
    /// - Source type is unsupported
    /// - Source initialization fails
    /// - Task spawning fails
    pub async fn spawn_source(
        &self,
        stream_id: String,
        source_config: SourceConfig,
    ) -> Result<String, CoreError>;

    /// Shutdown a specific source
    ///
    /// Sends shutdown signal and waits for task completion.
    ///
    /// # Arguments
    /// * `source_id` - Unique source identifier
    ///
    /// # Errors
    /// Returns error if source not found or shutdown fails
    pub async fn shutdown_source(&self, source_id: &str) -> Result<(), CoreError>;

    /// Shutdown all active sources
    ///
    /// Gracefully shuts down all sources in parallel.
    pub async fn shutdown_all(&self) -> Result<(), CoreError>;

    /// Check health of a specific source
    ///
    /// # Arguments
    /// * `source_id` - Source identifier
    ///
    /// # Returns
    /// Health status of the source
    pub async fn health_check(&self, source_id: &str) -> Result<SourceHealth, CoreError>;

    /// List all active source IDs
    pub async fn list_sources(&self) -> Vec<String>;

    /// Get count of active sources per stream
    pub async fn source_count_by_stream(&self) -> HashMap<String, usize>;
}

/// Health status for a source
#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceHealth {
    pub source_id: String,
    pub stream_id: String,
    pub source_type: SourceType,
    pub healthy: bool,
    pub message: String,
    pub details: HashMap<String, serde_json::Value>,
}

/// Stream registry event types
#[derive(Debug, Clone)]
pub enum StreamEvent {
    Added(String, StreamConfig),
    Updated(String, StreamConfig, StreamConfig),  // stream_id, old, new
    Deleted(String),
}
```

### Implementation Pattern

Follows the MqttHandler spawning pattern from main.rs:

```rust
impl SourceManager {
    pub async fn spawn_source(
        &self,
        stream_id: String,
        source_config: SourceConfig,
    ) -> Result<String, CoreError> {
        let source_id = format!("{}-{}", stream_id, source_config.id);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let router = Arc::clone(&self.router);

        // Pattern: match source type and create appropriate handler
        let task_handle = match source_config.source_type {
            SourceType::Mqtt => {
                // Reuse existing MqttHandler pattern (like main.rs)
                let mqtt_config = parse_mqtt_config(&source_config)?;
                let (tx, mut rx) = mpsc::channel(1000);

                // Spawn MqttHandler (existing code)
                let handler = MqttHandler::new(mqtt_config, tx).await?;

                tokio::spawn(async move {
                    tokio::select! {
                        result = handler.run() => result,
                        _ = shutdown_rx => {
                            tracing::info!("MQTT source {} shutting down", source_id);
                            Ok(())
                        }
                    }
                })
            }

            SourceType::HttpPoll => {
                // New HTTP polling source
                let http_config = parse_http_config(&source_config)?;
                let poller = HttpPoller::new(stream_id.clone(), http_config);

                tokio::spawn(async move {
                    tokio::select! {
                        result = poller.run() => result,
                        _ = shutdown_rx => {
                            tracing::info!("HTTP poller {} shutting down", source_id);
                            Ok(())
                        }
                    }
                })
            }

            SourceType::Webhook => {
                // New webhook handler
                let webhook_config = parse_webhook_config(&source_config)?;
                let handler = WebhookHandler::new(stream_id.clone(), webhook_config);

                tokio::spawn(async move {
                    tokio::select! {
                        result = handler.run() => result,
                        _ = shutdown_rx => {
                            tracing::info!("Webhook handler {} shutting down", source_id);
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
        };

        // Register handle (like storing task handles in main.rs)
        let handle = SourceHandle {
            source_id: source_id.clone(),
            stream_id,
            source_type: source_config.source_type,
            task_handle,
            shutdown_tx,
        };

        self.active_sources.write().await.insert(source_id.clone(), handle);

        Ok(source_id)
    }
}
```

---

## 3. IngestionRouter Interface

### Purpose
Validates incoming records against stream schemas and routes them to the appropriate storage writer channels.

### Design Pattern
Follows the "channel router" pattern. Maintains a mapping of stream_id -> channel for forwarding records.

### Interface

```rust
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use neural_core::{CoreError, StreamRecord};
use config_client::StreamRegistry;

/// Routes and validates records to storage writers
pub struct IngestionRouter {
    registry: Arc<StreamRegistry>,

    // Maps stream_id -> channel for storage writers
    storage_channels: Arc<RwLock<HashMap<String, mpsc::Sender<StreamRecord>>>>,

    // Dead-letter queue for invalid records
    dead_letter_tx: mpsc::Sender<DeadLetterItem>,
}

/// Record that failed validation
#[derive(Debug, Clone)]
pub struct DeadLetterItem {
    pub stream_id: String,
    pub source_id: String,
    pub record: StreamRecord,
    pub error: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl IngestionRouter {
    /// Create a new ingestion router
    ///
    /// # Arguments
    /// * `registry` - Stream registry for loading schemas
    pub fn new(registry: Arc<StreamRegistry>) -> Self;

    /// Register a storage writer channel for a stream
    ///
    /// This must be called before records can be routed to the stream.
    ///
    /// # Arguments
    /// * `stream_id` - Stream identifier
    /// * `tx` - Channel sender for the storage writer
    pub async fn register_writer(
        &self,
        stream_id: String,
        tx: mpsc::Sender<StreamRecord>,
    ) -> Result<(), CoreError>;

    /// Unregister a storage writer channel
    ///
    /// Called when a stream is deleted or writer is shutting down.
    ///
    /// # Arguments
    /// * `stream_id` - Stream identifier
    pub async fn unregister_writer(&self, stream_id: &str);

    /// Route a record to the appropriate storage writer
    ///
    /// This performs:
    /// 1. Schema validation against stream definition
    /// 2. Enrichment with metadata (stream_id, source_id tags)
    /// 3. Forwarding to storage writer channel
    ///
    /// Invalid records are sent to dead-letter queue.
    ///
    /// # Arguments
    /// * `source_id` - Source that produced the record
    /// * `stream_id` - Target stream
    /// * `record` - Record to route
    ///
    /// # Errors
    /// Returns error if routing fails (not for validation errors)
    pub async fn route_record(
        &self,
        source_id: &str,
        stream_id: &str,
        record: StreamRecord,
    ) -> Result<(), CoreError>;

    /// Validate a record against stream schema
    ///
    /// # Arguments
    /// * `stream_id` - Stream identifier
    /// * `record` - Record to validate
    ///
    /// # Errors
    /// Returns ValidationError with details
    async fn validate_record(
        &self,
        stream_id: &str,
        record: &StreamRecord,
    ) -> Result<(), ValidationError>;

    /// Get dead-letter queue receiver
    ///
    /// Consumers can monitor this channel for validation failures.
    pub fn dead_letter_receiver(&self) -> mpsc::Receiver<DeadLetterItem>;
}

/// Validation error details
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Required field '{field}' is missing")]
    RequiredFieldMissing { field: String },

    #[error("Field '{field}' type mismatch: expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },

    #[error("Field '{field}' value {value} out of range [{min}, {max}]")]
    OutOfRange {
        field: String,
        value: f64,
        min: f64,
        max: f64,
    },

    #[error("Field '{field}' value '{value}' not in allowed enum: {allowed:?}")]
    NotInEnum {
        field: String,
        value: String,
        allowed: Vec<String>,
    },
}
```

### Implementation Pattern

Follows the channel-based forwarding pattern from StorageWriter:

```rust
impl IngestionRouter {
    pub async fn route_record(
        &self,
        source_id: &str,
        stream_id: &str,
        mut record: StreamRecord,
    ) -> Result<(), CoreError> {
        // 1. Validate against schema
        if let Err(e) = self.validate_record(stream_id, &record).await {
            tracing::warn!(
                "Validation failed for stream {}: {}",
                stream_id,
                e
            );

            // Send to dead-letter queue (like error handling in main.rs)
            let _ = self.dead_letter_tx.send(DeadLetterItem {
                stream_id: stream_id.to_string(),
                source_id: source_id.to_string(),
                record,
                error: e.to_string(),
                timestamp: chrono::Utc::now(),
            }).await;

            return Ok(()); // Not an error, just invalid data
        }

        // 2. Enrich with metadata (like adding tags in MqttHandler)
        record.point.tags.insert(
            "stream_id".to_string(),
            stream_id.to_string()
        );
        record.point.tags.insert(
            "source_id".to_string(),
            source_id.to_string()
        );

        // 3. Route to storage writer (like sending through channel in main.rs)
        let channels = self.storage_channels.read().await;

        if let Some(tx) = channels.get(stream_id) {
            // Send to channel (like MqttHandler -> StorageWriter)
            tx.send(record).await.map_err(|e| {
                CoreError::Source(format!("Failed to route record: {}", e))
            })?;

            tracing::debug!("Routed record to stream {}", stream_id);
        } else {
            tracing::error!("No storage writer for stream: {}", stream_id);
            return Err(CoreError::Source(
                format!("No writer registered for stream: {}", stream_id)
            ));
        }

        Ok(())
    }
}
```

---

## 4. Shared Types

### Module: coordinator/types.rs

```rust
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Complete stream definition loaded from registry
#[derive(Debug, Clone)]
pub struct StreamDefinition {
    pub config: StreamConfig,
    pub schema: SchemaDefinition,
    pub sources: Vec<SourceConfig>,
}

/// Stream record with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamRecord {
    pub stream_id: String,
    pub point: TimeSeriesPoint,
    pub metadata: Option<RecordMetadata>,
}

impl StreamRecord {
    /// Create a new StreamRecord
    pub fn new(stream_id: String, point: TimeSeriesPoint) -> Self {
        Self {
            stream_id,
            point,
            metadata: None,
        }
    }

    /// Add source metadata
    pub fn with_metadata(
        mut self,
        source_id: String,
        source_type: String,
    ) -> Self {
        self.metadata = Some(RecordMetadata {
            source_id,
            source_type,
            ingestion_time: Utc::now(),
        });
        self
    }
}

/// Metadata about record ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordMetadata {
    pub source_id: String,
    pub source_type: String,
    pub ingestion_time: DateTime<Utc>,
}

/// Backward compatibility: convert TimeSeriesPoint -> StreamRecord
impl From<TimeSeriesPoint> for StreamRecord {
    fn from(point: TimeSeriesPoint) -> Self {
        Self {
            stream_id: "air-quality".to_string(), // Default for AIR-002 compatibility
            point,
            metadata: None,
        }
    }
}

/// Extract point from record (for storage layer)
impl From<StreamRecord> for TimeSeriesPoint {
    fn from(record: StreamRecord) -> Self {
        record.point
    }
}
```

---

## 5. Integration with Existing Components

### Reuse Patterns

#### 1. MqttHandler (AIR-002)
```rust
// Existing: apps/air-quality-app/src/ingestion/mqtt_handler.rs
// NO CHANGES REQUIRED - Used as-is by SourceManager

pub struct MqttHandler {
    source: MqttSource,
    sender: mpsc::Sender<TimeSeriesPoint>,
}

// SourceManager wraps this:
let handler = MqttHandler::new(mqtt_config, tx).await?;
tokio::spawn(async move { handler.run().await });
```

#### 2. StorageWriter (AIR-002)
```rust
// Existing: apps/air-quality-app/src/pipeline/storage_writer.rs
// MINIMAL CHANGES - Add stream_id parameter

pub struct StorageWriter {
    store: Arc<ParquetStore>,  // Will become Arc<StorageLayerManager>
    stream_id: String,         // NEW: identifies stream
    receiver: mpsc::Receiver<TimeSeriesPoint>,
    batch_size: usize,
    batch_timeout: Duration,
}

// IngestionCoordinator creates one per stream:
let writer = StorageWriter::new(
    layer_manager,
    stream_id.clone(),
    rx,
    Some(100),
    Some(Duration::from_secs(5)),
);
```

#### 3. StreamRegistry (AIR-004 Phase 1)
```rust
// New: config-client/src/stream/registry.rs
// Already implemented in Phase 1

pub struct StreamRegistry {
    client: ConfigClient,  // Reuses existing ConfigClient
}

// IngestionCoordinator uses for:
// - Loading stream definitions
// - Watching for changes
let streams = registry.list_streams().await?;
let watch_rx = registry.watch_streams().await?;
```

---

## 6. Error Handling Strategy

### Error Types

Following AIR-002 pattern of using `neural_core::CoreError`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoordinatorError {
    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Source initialization failed: {0}")]
    SourceInit(String),

    #[error("Writer initialization failed: {0}")]
    WriterInit(String),

    #[error("Registry error: {0}")]
    Registry(String),

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Core error: {0}")]
    Core(#[from] CoreError),
}

impl From<CoordinatorError> for CoreError {
    fn from(err: CoordinatorError) -> Self {
        CoreError::Source(err.to_string())
    }
}
```

### Error Handling Pattern

```rust
// Pattern: Log and continue for non-critical errors
match self.handle_stream_event(event).await {
    Ok(_) => {
        tracing::info!("Stream event handled successfully");
    }
    Err(e) => {
        // Don't crash coordinator for stream-specific errors
        tracing::error!("Failed to handle stream event: {}", e);
        // Metrics: increment error counter
        // Continue processing other streams
    }
}

// Pattern: Propagate critical errors
if let Err(e) = self.initialize().await {
    // Initialization failure is critical
    return Err(e);
}
```

---

## 7. Metrics and Observability

### Metrics Collection Points

Following AIR-002 tracing pattern:

```rust
use tracing::{debug, info, warn, error};

// Coordinator initialization
info!("Initializing IngestionCoordinator");
info!("Loaded {} streams from registry", stream_count);

// Source spawning
info!("Spawned {} source for stream {}", source_type, stream_id);
debug!("Source {} configuration: {:?}", source_id, config);

// Record routing
debug!("Routing record to stream {}", stream_id);
warn!("Validation failed for stream {}: {}", stream_id, error);

// Storage writes
info!("Flushed {} records to Bronze for stream {}", count, stream_id);
error!("Silver write failed for stream {}: {}", stream_id, error);

// Health checks
info!("Health check: {} streams active, {} sources healthy",
      stream_count, healthy_sources);
```

### Structured Logging

```rust
use tracing::instrument;

#[instrument(skip(self, record), fields(stream_id = %stream_id, source_id = %source_id))]
pub async fn route_record(
    &self,
    source_id: &str,
    stream_id: &str,
    record: StreamRecord,
) -> Result<(), CoreError> {
    // Automatic span tracking
}
```

---

## 8. Testing Strategy

### Unit Tests (London TDD)

Mock all dependencies:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        StreamRegistry {}

        #[async_trait]
        impl StreamRegistry {
            async fn load_stream(&self, stream_id: &str) -> Result<StreamDefinition>;
            async fn list_streams(&self) -> Result<Vec<String>>;
            async fn watch_streams(&self) -> Result<Receiver<StreamEvent>>;
        }
    }

    #[tokio::test]
    async fn test_coordinator_initialization() {
        let mut mock_registry = MockStreamRegistry::new();
        mock_registry
            .expect_list_streams()
            .returning(|| Ok(vec!["air-quality".to_string()]));

        // Test coordinator initialization with mock
    }

    #[tokio::test]
    async fn test_source_spawning() {
        // Mock SourceManager spawning
    }

    #[tokio::test]
    async fn test_record_routing() {
        // Mock IngestionRouter validation and routing
    }
}
```

### Integration Tests

Test with real components:

```rust
#[tokio::test]
#[ignore] // Run with --ignored when etcd available
async fn test_coordinator_full_pipeline() {
    // Start test etcd
    let etcd = testcontainers::etcd::Etcd::default();

    // Populate test stream
    // ... (use sync script)

    // Create coordinator
    let config = CoordinatorConfig {
        etcd_endpoints: vec![etcd.endpoint()],
        ..Default::default()
    };

    let mut coordinator = IngestionCoordinator::new(config).await?;

    // Test stream loading
    coordinator.initialize().await?;

    // Verify sources spawned
    assert_eq!(coordinator.source_count(), 1);

    // Test record flow (inject test message)
    // ...

    // Verify storage write
    // ...
}
```

---

## 9. Deployment Considerations

### Resource Requirements

Based on AIR-002 baseline:

| Component | Memory | CPU |
|-----------|--------|-----|
| IngestionCoordinator base | ~50MB | 5% |
| SourceManager (3 streams) | ~30MB | 5% |
| IngestionRouter | ~20MB | 5% |
| StorageWriter (3x) | ~60MB | 10% |
| **Total Overhead** | **~160MB** | **25%** |

**Raspberry Pi 5 Budget**: 896MB total, ~160MB for multi-stream = ~740MB remaining for existing components

### Configuration Example

```yaml
# apps/air-quality-app/config.yaml (extend existing)
coordinator:
  etcd_endpoints:
    - http://etcd:2379
  storage:
    base_path: /app/data
  timescale:
    enabled: false
    url: postgres://timescale:5432/neural
  defaults:
    buffer_capacity: 1000
    batch_size: 100
    batch_timeout_secs: 5
```

---

## 10. Migration Path

### Phase 1: Parallel Deployment (Week 1-2)

1. Deploy IngestionCoordinator alongside air-quality-app
2. Both ingest air-quality stream (validation)
3. Compare outputs for correctness

```yaml
# docker-compose.yml
services:
  air-quality-app:
    # Existing service (unchanged)

  ingestion-coordinator:
    # New service (parallel)
    depends_on:
      - etcd
      - air-quality-app
```

### Phase 2: Add New Streams (Week 3)

1. Add home-events and weather to coordinator
2. air-quality-app continues as-is
3. Validate multi-stream operation

### Phase 3: Cutover (Week 4)

1. Redirect AirGradient to coordinator
2. Keep air-quality-app on standby
3. Monitor for 48 hours

### Phase 4: Decommission (Week 5+)

1. Remove air-quality-app MQTT handler
2. Keep API components
3. Document migration

---

## References

- AIR-001: Hexagonal Architecture Foundation
- AIR-002: MQTT Ingestion Pipeline (MqttHandler, StorageWriter patterns)
- AIR-003: etcd Configuration Architecture (ConfigClient patterns)
- ADR-001: Multi-Stream Foundation
- ADR-002: Stream Registry Design
- IMPLEMENTATION_PLAN.md: Phase 4 detailed tasks

---

**Document Status**: Complete - Ready for Implementation
**Next Review**: After SourceManager implementation
**Maintained By**: ARCHITECT agent
**Last Updated**: 2025-12-15
