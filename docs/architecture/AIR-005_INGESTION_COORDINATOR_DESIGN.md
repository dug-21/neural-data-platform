# AIR-005 IngestionCoordinator Architecture Design

**Version**: 1.1.0
**Date**: 2025-12-16
**Author**: System Architect
**Status**: Partial Implementation - HTTP Polling Complete

---

## Table of Contents

1. [Overview](#overview)
2. [Architectural Decisions](#architectural-decisions)
3. [Component Design](#component-design)
4. [Trait Definitions](#trait-definitions)
5. [Struct Layouts](#struct-layouts)
6. [Interaction Diagrams](#interaction-diagrams)
7. [Channel Ownership Model](#channel-ownership-model)
8. [Error Handling Strategy](#error-handling-strategy)
9. [Shutdown Coordination](#shutdown-coordination)
10. [Integration Points](#integration-points)

---

## 1. Overview

### 1.1 Problem Statement

The current architecture (AIR-004) has:
- Sources directly writing to a single channel
- No dynamic source lifecycle management
- Manual source initialization in main.rs
- No coordination layer between sources and storage

AIR-005 introduces:
- **SourceManager**: Dynamically spawns/stops sources based on etcd StreamRegistry (DESIGN COMPLETE)
- **IngestionCoordinator**: Central coordination point owning the mpsc channel (DESIGN COMPLETE)
- **IngestionRouter**: Routes data to appropriate storage writers (IMPLEMENTED IN AIR-004)
- **GenericHttpPollingSource**: Generic HTTP source with pluggable parsers (IMPLEMENTED)
- **ParserRegistry**: Plugin system for response parsers (IMPLEMENTED)
- **OpenWeatherMap Integration**: Weather and air pollution parsers (IMPLEMENTED)

### 1.2 Design Goals

1. **Dynamic Source Management**: Sources can be added/removed without restart
2. **Centralized Channel Ownership**: Clear ownership model for coordination
3. **Clean Separation of Concerns**: Source lifecycle vs. data routing
4. **Backward Compatible**: Existing MqttSource, StorageWriter unchanged
5. **Testable**: London School TDD with trait-based mocks

### 1.3 Implementation Status (2025-12-16)

**COMPLETED**:
- ✅ Generic HTTP Polling Source (`core/src/sources/http_poll.rs`)
- ✅ ResponseParser trait for pluggable parsers
- ✅ AuthMethod enum (None, QueryParam, Header, Bearer)
- ✅ RetryConfig with exponential backoff and jitter
- ✅ EndpointConfig for flexible endpoint definitions
- ✅ ParserRegistry for parser management
- ✅ WeatherParser for OpenWeatherMap current weather API (`core/src/sources/parsers/weather.rs`)
- ✅ AirPollutionParser for OpenWeatherMap air pollution API (`core/src/sources/parsers/air_pollution.rs`)
- ✅ Stream configurations created:
  - `config/streams/outdoor-weather.yaml`
  - `config/streams/outdoor-air-quality.yaml`

**IN PROGRESS**:
- 🚧 SourceManager implementation
- 🚧 IngestionCoordinator implementation

**NOT STARTED**:
- ⏳ Factory pattern for source creation
- ⏳ etcd watch integration for hot-reload
- ⏳ Circuit breaker for source health

---

## 2. Architectural Decisions

### ADR-001: IngestionCoordinator Owns Master Channel

**Context**: Need clear ownership of the mpsc channel between sources and storage.

**Decision**: IngestionCoordinator creates and owns the master `mpsc::Sender<TimeSeriesPoint>`.

**Rationale**:
- Single source of truth for channel lifecycle
- Sources get clones of the sender
- Coordinator controls backpressure and buffer sizing
- Clean shutdown by dropping the sender

**Consequences**:
- SourceManager gets sender clones from coordinator
- Sources cannot outlive the coordinator
- Shutdown coordination is simplified

---

### ADR-002: SourceManager Uses Trait-Based Spawning

**Context**: Need to spawn different source types (MQTT, HTTP, WebSocket) dynamically.

**Decision**: Introduce `SourceFactory` trait for creating sources.

**Rationale**:
- Type-safe spawning without Box<dyn Source>
- Support for different source configurations
- Easy to test with mock factories
- Future-proof for new source types

**Consequences**:
- Each source type needs a factory implementation
- Some boilerplate code required
- Clear separation between creation and lifecycle management

---

### ADR-003: Graceful Shutdown via Tokio CancellationToken

**Context**: Need coordinated shutdown across multiple sources and writers.

**Decision**: Use `tokio_util::sync::CancellationToken` for shutdown signaling.

**Rationale**:
- Tokio-native shutdown pattern
- Composable (child tokens)
- Non-blocking notification
- Works across task boundaries

**Consequences**:
- Add tokio-util dependency
- All source tasks must monitor cancellation token
- Requires task join handles for graceful completion

---

### ADR-004: Registry Watcher Runs in SourceManager

**Context**: Need to react to etcd stream config changes.

**Decision**: SourceManager spawns background task to watch StreamRegistry.

**Rationale**:
- Separates watching logic from coordinator
- Allows hot-reload of sources
- Isolated error handling for watch failures

**Consequences**:
- SourceManager is long-lived (not just spawn/stop)
- Need internal task cancellation on shutdown
- Potential race conditions during rapid config changes

---

## 3. Component Design

### 3.1 Component Hierarchy

```
┌──────────────────────────────────────────────────────────────────┐
│                     air-quality-app::main                        │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │           IngestionCoordinator (NEW)                       │ │
│  │  - Owns master mpsc::Sender<TimeSeriesPoint>              │ │
│  │  - Owns SourceManager                                      │ │
│  │  - Owns IngestionRouter                                    │ │
│  │  - Orchestrates startup/shutdown                           │ │
│  └─────────────────┬──────────────────────────────────────────┘ │
│                    │                                             │
│     ┌──────────────┴──────────────┐                            │
│     ▼                              ▼                             │
│  ┌──────────────────────┐   ┌────────────────────────────────┐ │
│  │  SourceManager (NEW) │   │  IngestionRouter (EXISTS)      │ │
│  │  - Watches etcd      │   │  - Schema validation           │ │
│  │  - Spawns sources    │   │  - Routes to storage channels  │ │
│  │  - Manages lifecycle │   │  - Dead letter queue           │ │
│  └──────┬───────────────┘   └────────────────────────────────┘ │
│         │                                                        │
│         ├──────────┬─────────────┬─────────────┐               │
│         ▼          ▼             ▼             ▼                │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │  MQTT    │  │  HTTP    │  │  HTTP    │  │ Future   │      │
│  │  Source  │  │  Weather │  │  AirQual │  │ Sources  │      │
│  │  Task    │  │  Task    │  │  Task    │  │          │      │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └──────────┘      │
│       │             │              │                            │
│       └─────────────┴──────────────┘                           │
│                     │                                           │
│                     ▼                                           │
│          mpsc::channel<TimeSeriesPoint>                        │
│                     │                                           │
│                     ▼                                           │
│          ┌────────────────────────┐                            │
│          │   StorageWriter        │                            │
│          │   (per-stream writers) │                            │
│          └────────────────────────┘                            │
└──────────────────────────────────────────────────────────────────┘
```

### 3.2 Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│  STARTUP SEQUENCE                                                │
└─────────────────────────────────────────────────────────────────┘

1. main() creates IngestionCoordinator
   ├─ Coordinator creates mpsc::channel (buffer: 10000)
   ├─ Coordinator creates SourceManager(sender.clone())
   ├─ Coordinator creates IngestionRouter
   └─ Coordinator starts background tasks

2. SourceManager::start()
   ├─ Spawns registry watcher task
   ├─ Loads initial stream configs from etcd
   └─ For each enabled stream with sources:
       ├─ Create SourceFactory for source type
       ├─ Spawn source task with sender.clone()
       └─ Store task handle in HashMap<stream_id, SourceHandle>

3. Sources produce data
   ├─ MQTT/HTTP tasks poll for data
   ├─ Parse into TimeSeriesPoint
   └─ sender.send(point).await

4. IngestionRouter receives from channel
   ├─ Validate against StreamConfig schema
   ├─ Enrich with metadata tags
   └─ Route to appropriate storage writer

┌─────────────────────────────────────────────────────────────────┐
│  RUNTIME CONFIGURATION CHANGES                                   │
└─────────────────────────────────────────────────────────────────┘

1. etcd watch detects change
   └─ StreamRegistry notifies SourceManager

2. SourceManager::handle_config_change()
   ├─ If stream disabled: stop_source(stream_id)
   │   ├─ Cancel source task (CancellationToken)
   │   ├─ Wait for task join
   │   └─ Remove from active_sources HashMap
   │
   ├─ If stream enabled: spawn_source(stream_id)
   │   ├─ Load StreamConfig from registry
   │   ├─ Create SourceFactory
   │   ├─ Spawn source task
   │   └─ Store task handle
   │
   └─ If source config changed: restart_source(stream_id)
       ├─ stop_source(stream_id)
       └─ spawn_source(stream_id)

┌─────────────────────────────────────────────────────────────────┐
│  SHUTDOWN SEQUENCE                                               │
└─────────────────────────────────────────────────────────────────┘

1. main() receives SIGINT/SIGTERM
   └─ Calls coordinator.shutdown().await

2. IngestionCoordinator::shutdown()
   ├─ Cancel global CancellationToken
   ├─ Call source_manager.stop_all().await
   │   ├─ For each source: cancel task
   │   └─ Wait for all join handles
   ├─ Drop master sender (signals end to router)
   └─ Wait for router task to complete

3. IngestionRouter task
   ├─ Receives None from channel (sender dropped)
   ├─ Flush any pending data
   └─ Exit gracefully

4. StorageWriter tasks
   ├─ Receive None from their channels
   ├─ Flush batches
   ├─ Sync WAL
   └─ Exit gracefully
```

---

## 4. Trait Definitions

### 4.1 SourceFactory Trait

**Location**: `core/src/coordinator/source_factory.rs`

```rust
use crate::error::CoreResult;
use crate::traits::TimeSeriesPoint;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Factory for creating and starting source tasks
#[async_trait]
pub trait SourceFactory: Send + Sync + 'static {
    /// Spawn a new source task that sends data to the channel
    ///
    /// # Arguments
    /// * `stream_id` - Unique identifier for the stream
    /// * `config` - Source-specific configuration (from StreamConfig.sources[].params)
    /// * `sender` - Channel sender for produced TimeSeriesPoints
    /// * `cancel_token` - Token to signal task cancellation
    ///
    /// # Returns
    /// Task join handle for graceful shutdown coordination
    async fn spawn(
        &self,
        stream_id: String,
        config: serde_json::Value,
        sender: mpsc::Sender<TimeSeriesPoint>,
        cancel_token: CancellationToken,
    ) -> CoreResult<tokio::task::JoinHandle<CoreResult<()>>>;

    /// Name of this factory for logging
    fn name(&self) -> &'static str;
}
```

**Design Notes**:
- `async_trait` for async spawn method
- Accepts arbitrary JSON config (parsed from StreamConfig)
- Returns `JoinHandle` for shutdown coordination
- `CancellationToken` for graceful cancellation

---

### 4.2 SourceHandle Struct

**Location**: `core/src/coordinator/source_manager.rs`

```rust
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use crate::error::CoreResult;

/// Handle to a running source task
pub struct SourceHandle {
    pub stream_id: String,
    pub source_type: String,
    pub cancel_token: CancellationToken,
    pub task_handle: JoinHandle<CoreResult<()>>,
}

impl SourceHandle {
    /// Cancel the source task and wait for completion
    pub async fn stop(self) -> CoreResult<()> {
        self.cancel_token.cancel();

        match self.task_handle.await {
            Ok(Ok(())) => {
                tracing::info!("Source {} stopped gracefully", self.stream_id);
                Ok(())
            }
            Ok(Err(e)) => {
                tracing::error!("Source {} stopped with error: {}", self.stream_id, e);
                Err(e)
            }
            Err(e) => {
                tracing::error!("Source {} task panicked: {}", self.stream_id, e);
                Err(CoreError::Source(format!("Task panic: {}", e)))
            }
        }
    }
}
```

---

## 5. Struct Layouts

### 5.1 IngestionCoordinator

**Location**: `apps/air-quality-app/src/coordinator/ingestion_coordinator.rs`

```rust
use neural_core::TimeSeriesPoint;
use config_client::StreamRegistry;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use std::sync::Arc;
use super::source_manager::SourceManager;
use super::router::IngestionRouter;
use crate::error::Result;

/// Central coordinator for data ingestion pipeline
///
/// Responsibilities:
/// - Create and own the master mpsc channel
/// - Initialize SourceManager and IngestionRouter
/// - Coordinate graceful startup and shutdown
pub struct IngestionCoordinator {
    /// Master channel sender (sources get clones)
    sender: mpsc::Sender<TimeSeriesPoint>,

    /// Master channel receiver (passed to router)
    receiver: Option<mpsc::Receiver<TimeSeriesPoint>>,

    /// Manages source lifecycle (spawn/stop/watch)
    source_manager: SourceManager,

    /// Routes data to storage channels
    router: Arc<IngestionRouter>,

    /// Global cancellation token for shutdown
    cancel_token: CancellationToken,

    /// Router task handle
    router_task: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl IngestionCoordinator {
    /// Create a new coordinator
    ///
    /// # Arguments
    /// * `registry` - StreamRegistry for loading stream configs
    /// * `buffer_capacity` - Size of master channel buffer (default: 10000)
    pub fn new(
        registry: Arc<StreamRegistry>,
        buffer_capacity: usize,
    ) -> Result<Self> {
        // Create master channel
        let (sender, receiver) = mpsc::channel(buffer_capacity);

        // Create dead letter channel for invalid data
        let (dead_letter_tx, dead_letter_rx) = mpsc::channel(1000);

        // Create router
        let router = Arc::new(IngestionRouter::new(
            registry.clone(),
            dead_letter_tx,
        ));

        // Create source manager
        let source_manager = SourceManager::new(
            registry.clone(),
            sender.clone(),
        );

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        Ok(Self {
            sender,
            receiver: Some(receiver),
            source_manager,
            router,
            cancel_token,
            router_task: None,
        })
    }

    /// Start the coordinator (spawn all background tasks)
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("Starting IngestionCoordinator");

        // Start source manager (spawns registry watcher and initial sources)
        self.source_manager
            .start(self.cancel_token.clone())
            .await?;

        // Spawn router task
        let receiver = self.receiver.take()
            .ok_or_else(|| anyhow::anyhow!("Coordinator already started"))?;

        let router = self.router.clone();
        let cancel_token = self.cancel_token.clone();

        let router_task = tokio::spawn(async move {
            router.run(receiver, cancel_token).await
        });

        self.router_task = Some(router_task);

        tracing::info!("IngestionCoordinator started successfully");
        Ok(())
    }

    /// Gracefully shutdown the coordinator
    pub async fn shutdown(mut self) -> Result<()> {
        tracing::info!("Shutting down IngestionCoordinator");

        // Cancel all tasks
        self.cancel_token.cancel();

        // Stop all sources first
        self.source_manager.stop_all().await?;

        // Drop master sender to signal router shutdown
        drop(self.sender);

        // Wait for router task to complete
        if let Some(task) = self.router_task {
            match task.await {
                Ok(Ok(())) => tracing::info!("Router stopped gracefully"),
                Ok(Err(e)) => tracing::error!("Router stopped with error: {}", e),
                Err(e) => tracing::error!("Router task panicked: {}", e),
            }
        }

        tracing::info!("IngestionCoordinator shutdown complete");
        Ok(())
    }

    /// Get a clone of the sender for testing or manual source creation
    pub fn sender(&self) -> mpsc::Sender<TimeSeriesPoint> {
        self.sender.clone()
    }

    /// Get router reference for registering storage channels
    pub fn router(&self) -> Arc<IngestionRouter> {
        self.router.clone()
    }
}
```

**Key Design Points**:
- **Channel Ownership**: Coordinator owns both sender and receiver
- **Option<Receiver>**: Taken when router starts (enforces single start)
- **Router Task Handle**: Stored for shutdown coordination
- **CancellationToken**: Shared across all components

---

### 5.2 SourceManager

**Location**: `core/src/coordinator/source_manager.rs`

```rust
use crate::error::{CoreError, CoreResult};
use crate::traits::TimeSeriesPoint;
use config_client::StreamRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use super::source_factory::SourceFactory;

/// Manages lifecycle of data sources based on StreamRegistry
pub struct SourceManager {
    /// Registry for loading stream configurations
    registry: Arc<StreamRegistry>,

    /// Master channel sender (cloned to each source)
    sender: mpsc::Sender<TimeSeriesPoint>,

    /// Currently running sources
    active_sources: Arc<RwLock<HashMap<String, SourceHandle>>>,

    /// Registry of source factories (MQTT, HTTP, etc.)
    factories: Arc<RwLock<HashMap<String, Arc<dyn SourceFactory>>>>,

    /// Watcher task handle
    watcher_task: Option<tokio::task::JoinHandle<CoreResult<()>>>,
}

impl SourceManager {
    /// Create a new source manager
    pub fn new(
        registry: Arc<StreamRegistry>,
        sender: mpsc::Sender<TimeSeriesPoint>,
    ) -> Self {
        let factories = Arc::new(RwLock::new(HashMap::new()));

        Self {
            registry,
            sender,
            active_sources: Arc::new(RwLock::new(HashMap::new())),
            factories,
            watcher_task: None,
        }
    }

    /// Register a source factory
    pub async fn register_factory(
        &self,
        source_type: String,
        factory: Arc<dyn SourceFactory>,
    ) {
        let mut factories = self.factories.write().await;
        factories.insert(source_type, factory);
    }

    /// Start the source manager
    ///
    /// This will:
    /// 1. Spawn registry watcher
    /// 2. Load initial stream configs
    /// 3. Spawn sources for enabled streams
    pub async fn start(&mut self, cancel_token: CancellationToken) -> CoreResult<()> {
        tracing::info!("Starting SourceManager");

        // Load initial stream configs
        let streams = self.registry.list_streams().await?;

        for stream_id in streams {
            if let Err(e) = self.spawn_source(&stream_id).await {
                tracing::error!("Failed to spawn source for {}: {}", stream_id, e);
                // Continue with other sources
            }
        }

        // Spawn registry watcher
        let registry = self.registry.clone();
        let active_sources = self.active_sources.clone();
        let factories = self.factories.clone();
        let sender = self.sender.clone();
        let cancel = cancel_token.clone();

        let watcher_task = tokio::spawn(async move {
            Self::watch_registry(
                registry,
                active_sources,
                factories,
                sender,
                cancel,
            ).await
        });

        self.watcher_task = Some(watcher_task);

        tracing::info!("SourceManager started successfully");
        Ok(())
    }

    /// Spawn a source for a stream
    async fn spawn_source(&self, stream_id: &str) -> CoreResult<()> {
        // Load stream config
        let config = self.registry.load_stream(stream_id).await?;

        // Check if stream is enabled
        if !config.enabled {
            tracing::debug!("Stream {} is disabled, skipping source spawn", stream_id);
            return Ok(());
        }

        // Get source configs (may have multiple sources per stream)
        for source_config in &config.sources {
            if !source_config.enabled {
                continue;
            }

            let source_type = format!("{:?}", source_config.source_type).to_lowercase();

            // Get factory
            let factories = self.factories.read().await;
            let factory = factories.get(&source_type)
                .ok_or_else(|| CoreError::Source(
                    format!("No factory registered for source type: {}", source_type)
                ))?
                .clone();
            drop(factories);

            // Create cancellation token for this source
            let cancel_token = CancellationToken::new();

            // Spawn source task
            let task_handle = factory.spawn(
                stream_id.to_string(),
                serde_json::to_value(&source_config.params)?,
                self.sender.clone(),
                cancel_token.clone(),
            ).await?;

            // Store handle
            let handle = SourceHandle {
                stream_id: stream_id.to_string(),
                source_type: source_type.clone(),
                cancel_token,
                task_handle,
            };

            let mut active = self.active_sources.write().await;
            active.insert(stream_id.to_string(), handle);

            tracing::info!("Spawned {} source for stream {}", source_type, stream_id);
        }

        Ok(())
    }

    /// Stop a source
    async fn stop_source(&self, stream_id: &str) -> CoreResult<()> {
        let mut active = self.active_sources.write().await;

        if let Some(handle) = active.remove(stream_id) {
            tracing::info!("Stopping source for stream {}", stream_id);
            handle.stop().await?;
        }

        Ok(())
    }

    /// Stop all sources
    pub async fn stop_all(&self) -> CoreResult<()> {
        tracing::info!("Stopping all sources");

        let mut active = self.active_sources.write().await;
        let handles: Vec<_> = active.drain().collect();
        drop(active);

        for (stream_id, handle) in handles {
            if let Err(e) = handle.stop().await {
                tracing::error!("Failed to stop source {}: {}", stream_id, e);
            }
        }

        // Cancel watcher task
        if let Some(task) = self.watcher_task.take() {
            task.abort();
            let _ = task.await;
        }

        tracing::info!("All sources stopped");
        Ok(())
    }

    /// Watch for registry changes
    async fn watch_registry(
        registry: Arc<StreamRegistry>,
        active_sources: Arc<RwLock<HashMap<String, SourceHandle>>>,
        factories: Arc<RwLock<HashMap<String, Arc<dyn SourceFactory>>>>,
        sender: mpsc::Sender<TimeSeriesPoint>,
        cancel_token: CancellationToken,
    ) -> CoreResult<()> {
        tracing::info!("Starting registry watcher");

        // TODO: Implement actual watch logic when StreamRegistry supports it
        // For now, just poll periodically

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    tracing::info!("Registry watcher cancelled");
                    break;
                }
                _ = interval.tick() => {
                    // Check for config changes
                    // This is a placeholder - real implementation would use etcd watch
                    tracing::debug!("Registry watcher tick (polling mode)");
                }
            }
        }

        Ok(())
    }
}
```

**Key Design Points**:
- **Factory Registry**: Plugin-style source registration
- **Active Sources Map**: Tracks running source tasks by stream_id
- **Watch Task**: Monitors etcd for config changes
- **Graceful Stop**: Cancels tasks and waits for completion

---

### 5.3 IngestionRouter Update

**Location**: `apps/air-quality-app/src/coordinator/router.rs` (EXISTING - ADD METHOD)

```rust
impl IngestionRouter {
    /// Run the router (consume from master channel, route to storage)
    ///
    /// This method runs indefinitely until:
    /// - The sender is dropped (channel closes)
    /// - Cancellation token is triggered
    pub async fn run(
        &self,
        mut receiver: mpsc::Receiver<TimeSeriesPoint>,
        cancel_token: CancellationToken,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Starting IngestionRouter");

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    tracing::info!("IngestionRouter cancelled");
                    break;
                }
                point = receiver.recv() => {
                    match point {
                        Some(p) => {
                            // Extract stream_id from tags (added by source)
                            let stream_id = p.tags.get("stream_id")
                                .ok_or("Missing stream_id tag")?;

                            let source_id = p.tags.get("source_id")
                                .unwrap_or(&"unknown".to_string())
                                .clone();

                            if let Err(e) = self.route_point(&source_id, stream_id, p).await {
                                tracing::error!("Failed to route point: {}", e);
                            }
                        }
                        None => {
                            tracing::info!("Master channel closed, shutting down router");
                            break;
                        }
                    }
                }
            }
        }

        tracing::info!("IngestionRouter stopped");
        Ok(())
    }
}
```

---

## 6. Channel Ownership Model

### 6.1 Master Channel

```rust
// Created by IngestionCoordinator
let (master_tx, master_rx) = mpsc::channel<TimeSeriesPoint>(10000);

// Ownership:
// - IngestionCoordinator owns master_tx
// - IngestionRouter owns master_rx (moved during start)

// Cloning:
// - SourceManager gets master_tx.clone()
// - Each source task gets master_tx.clone()
```

### 6.2 Storage Channels

```rust
// Created per-stream by main.rs
let (storage_tx, storage_rx) = mpsc::channel<TimeSeriesPoint>(1000);

// Ownership:
// - IngestionRouter holds storage_tx (in HashMap)
// - StorageWriter owns storage_rx

// Lifecycle:
// - Registered via router.register_storage_channel()
// - Unregistered when stream disabled
```

### 6.3 Dead Letter Channel

```rust
// Created by IngestionCoordinator
let (dead_letter_tx, dead_letter_rx) = mpsc::channel<DeadLetterItem>(1000);

// Ownership:
// - IngestionRouter holds dead_letter_tx
// - DeadLetterHandler owns dead_letter_rx (future feature)
```

### 6.4 Diagram: Channel Ownership

```
┌─────────────────────────────────────────────────────────────┐
│                   IngestionCoordinator                      │
│                                                             │
│  master_tx: Sender<TimeSeriesPoint>  [OWNS]               │
│  master_rx: Receiver<TimeSeriesPoint> [GIVES TO ROUTER]   │
│                                                             │
└──────────────┬────────────────────────────┬─────────────────┘
               │                            │
               ├─ clone() ──────────────────┼──> SourceManager
               │                            │
               └─ move() ──────────────────>│
                                            │
                                            ▼
                                  ┌──────────────────────┐
                                  │  IngestionRouter     │
                                  │                      │
                                  │  master_rx [OWNS]   │
                                  │                      │
                                  │  storage_channels:   │
                                  │    HashMap<          │
                                  │      stream_id,      │
                                  │      Sender [REFS]   │
                                  │    >                 │
                                  └──────────────────────┘
                                            │
                                            ├──> storage_tx[stream1] ──> StorageWriter1
                                            ├──> storage_tx[stream2] ──> StorageWriter2
                                            └──> storage_tx[streamN] ──> StorageWriterN
```

---

## 7. Error Handling Strategy

### 7.1 Error Classification

```rust
pub enum CoordinatorError {
    /// Source failed to start
    SourceStartup { stream_id: String, source: CoreError },

    /// Source task panicked or crashed
    SourceCrashed { stream_id: String, message: String },

    /// Registry unavailable or corrupted
    RegistryError { operation: String, source: ConfigError },

    /// Channel full (backpressure)
    ChannelFull { stream_id: String },

    /// Invalid configuration
    InvalidConfig { stream_id: String, reason: String },
}
```

### 7.2 Error Handling Policy

| Error Type | Action | Propagate? | Retry? |
|------------|--------|-----------|--------|
| SourceStartup | Log error, continue with other sources | No | No |
| SourceCrashed | Log error, attempt restart after delay | No | Yes (3x) |
| RegistryError | Log error, use last known config | No | Yes (indefinite) |
| ChannelFull | Apply backpressure, slow source polling | No | Implicit |
| InvalidConfig | Log error, skip stream | No | No |

### 7.3 Circuit Breaker

```rust
/// Track source health for circuit breaker pattern
struct SourceHealth {
    consecutive_failures: u32,
    last_failure: Option<Instant>,
    state: CircuitState,
}

enum CircuitState {
    Closed,          // Normal operation
    Open,            // Too many failures, stop trying
    HalfOpen,        // Testing if recovered
}

impl SourceManager {
    async fn spawn_source_with_circuit_breaker(
        &self,
        stream_id: &str,
    ) -> CoreResult<()> {
        let health = self.get_health(stream_id);

        match health.state {
            CircuitState::Open => {
                // Check if cooldown period has passed
                if health.last_failure.elapsed() > Duration::from_secs(60) {
                    health.state = CircuitState::HalfOpen;
                } else {
                    return Err(CoreError::CircuitOpen);
                }
            }
            _ => {}
        }

        match self.spawn_source_internal(stream_id).await {
            Ok(()) => {
                health.consecutive_failures = 0;
                health.state = CircuitState::Closed;
                Ok(())
            }
            Err(e) => {
                health.consecutive_failures += 1;
                health.last_failure = Some(Instant::now());

                if health.consecutive_failures >= 5 {
                    health.state = CircuitState::Open;
                }

                Err(e)
            }
        }
    }
}
```

---

## 8. Shutdown Coordination

### 8.1 Shutdown Sequence

```
┌─────────────────────────────────────────────────────────────┐
│  GRACEFUL SHUTDOWN PROTOCOL                                 │
└─────────────────────────────────────────────────────────────┘

1. Signal received (SIGINT/SIGTERM)
   ↓
2. coordinator.shutdown() called
   ↓
3. Global CancellationToken.cancel()
   ├─ Notifies all source tasks
   ├─ Notifies router task
   └─ Notifies watcher task
   ↓
4. source_manager.stop_all()
   ├─ For each source:
   │   ├─ cancel_token.cancel()
   │   ├─ Wait for task.await (max 5s timeout)
   │   └─ If timeout: task.abort()
   └─ All sources stopped
   ↓
5. Drop master_tx
   ├─ Closes master channel
   └─ Signals router to flush and exit
   ↓
6. router_task.await
   ├─ Processes remaining points in channel
   ├─ Flushes to storage channels
   └─ Returns Ok(())
   ↓
7. Storage writers receive channel close
   ├─ Flush pending batches
   ├─ Sync WAL
   └─ Exit
   ↓
8. Coordinator::shutdown() returns
   └─ main() exits

Timeouts:
- Source task join: 5 seconds
- Router flush: 10 seconds
- Total shutdown: 30 seconds (then force exit)
```

### 8.2 Implementation

```rust
impl IngestionCoordinator {
    pub async fn shutdown(mut self) -> Result<()> {
        tracing::info!("Initiating graceful shutdown");

        // Step 1: Cancel all tasks
        self.cancel_token.cancel();

        // Step 2: Stop sources with timeout
        let stop_result = tokio::time::timeout(
            Duration::from_secs(5),
            self.source_manager.stop_all()
        ).await;

        match stop_result {
            Ok(Ok(())) => tracing::info!("All sources stopped gracefully"),
            Ok(Err(e)) => tracing::error!("Error stopping sources: {}", e),
            Err(_) => tracing::error!("Source shutdown timed out after 5s"),
        }

        // Step 3: Close master channel
        drop(self.sender);

        // Step 4: Wait for router with timeout
        if let Some(task) = self.router_task {
            let router_result = tokio::time::timeout(
                Duration::from_secs(10),
                task
            ).await;

            match router_result {
                Ok(Ok(Ok(()))) => tracing::info!("Router stopped gracefully"),
                Ok(Ok(Err(e))) => tracing::error!("Router error: {}", e),
                Ok(Err(e)) => tracing::error!("Router task panicked: {}", e),
                Err(_) => {
                    tracing::error!("Router shutdown timed out after 10s");
                }
            }
        }

        tracing::info!("Shutdown complete");
        Ok(())
    }
}
```

---

## 9. Integration Points

### 9.1 main.rs Changes

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... tracing setup ...

    // Load etcd endpoint
    let etcd_endpoint = std::env::var("ETCD_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:2379".to_string());

    // Create StreamRegistry
    let registry = Arc::new(
        StreamRegistry::new(&[&etcd_endpoint]).await?
    );

    // Create IngestionCoordinator
    let mut coordinator = IngestionCoordinator::new(
        registry.clone(),
        10000, // buffer capacity
    )?;

    // Register source factories
    coordinator.source_manager()
        .register_factory(
            "mqtt".to_string(),
            Arc::new(MqttSourceFactory::new()),
        ).await;

    coordinator.source_manager()
        .register_factory(
            "http_poll".to_string(),
            Arc::new(HttpPollSourceFactory::new()),
        ).await;

    // Create storage writers (per-stream)
    let store = Arc::new(ParquetStore::new(&config.storage.base_path)?);

    for stream_id in registry.list_streams().await? {
        let (storage_tx, storage_rx) = mpsc::channel(1000);

        // Register with router
        coordinator.router()
            .register_storage_channel(stream_id.clone(), storage_tx)
            .await;

        // Spawn storage writer
        let store_clone = store.clone();
        tokio::spawn(async move {
            let writer = StorageWriter::new(
                store_clone,
                storage_rx,
                Some(100),
                Some(Duration::from_secs(5)),
            );

            if let Err(e) = writer.run().await {
                tracing::error!("Storage writer for {} failed: {}", stream_id, e);
            }
        });
    }

    // Start coordinator
    coordinator.start().await?;

    // ... API server setup ...

    // Shutdown handling
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received shutdown signal");
            coordinator.shutdown().await?;
        }
    }

    Ok(())
}
```

### 9.2 Factory Implementations

```rust
// core/src/sources/mqtt_factory.rs
pub struct MqttSourceFactory;

#[async_trait]
impl SourceFactory for MqttSourceFactory {
    async fn spawn(
        &self,
        stream_id: String,
        config: serde_json::Value,
        sender: mpsc::Sender<TimeSeriesPoint>,
        cancel_token: CancellationToken,
    ) -> CoreResult<JoinHandle<CoreResult<()>>> {
        // Parse config
        let mqtt_config: MqttConfig = serde_json::from_value(config)?;

        // Spawn task
        let task = tokio::spawn(async move {
            let mut source = MqttSource::new(mqtt_config);
            source.start().await?;

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        source.stop().await?;
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        // Fetch and send points
                        let points = source.fetch().await?;
                        for mut point in points {
                            // Add stream metadata
                            point.tags.insert("stream_id".to_string(), stream_id.clone());
                            point.tags.insert("source_id".to_string(), "mqtt".to_string());

                            sender.send(point).await
                                .map_err(|e| CoreError::Source(format!("Send failed: {}", e)))?;
                        }
                    }
                }
            }

            Ok(())
        });

        Ok(task)
    }

    fn name(&self) -> &'static str {
        "mqtt"
    }
}
```

---

## 10. Memory Considerations

### 10.1 Memory Budget

```
Component                    Memory Estimate
─────────────────────────────────────────────
IngestionCoordinator         ~1 KB (struct overhead)
SourceManager                ~5 KB (HashMap overhead)
IngestionRouter              ~10 KB (HashMap + cache)
Active Sources (3x)          ~300 KB (MQTT clients)
Master Channel (10000 cap)   ~2 MB (TimeSeriesPoint = 200 bytes)
Storage Channels (3x 1000)   ~600 KB
Dead Letter Queue (1000)     ~200 KB
─────────────────────────────────────────────
Total Coordinator Overhead   ~3.1 MB
```

**Impact**: Minimal impact on 512MB container limit (< 1% increase)

---

## 11. Testing Strategy

### 11.1 Unit Tests (London School TDD)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        SourceFactory {}

        #[async_trait]
        impl SourceFactory for SourceFactory {
            async fn spawn(
                &self,
                stream_id: String,
                config: serde_json::Value,
                sender: mpsc::Sender<TimeSeriesPoint>,
                cancel_token: CancellationToken,
            ) -> CoreResult<JoinHandle<CoreResult<()>>>;

            fn name(&self) -> &'static str;
        }
    }

    #[tokio::test]
    async fn test_coordinator_starts_sources() {
        // Mock registry
        let registry = Arc::new(MockStreamRegistry::new());
        registry.expect_list_streams()
            .returning(|| Ok(vec!["test-stream".to_string()]));

        // Mock factory
        let mut factory = MockSourceFactory::new();
        factory.expect_spawn()
            .times(1)
            .returning(|_, _, _, _| {
                Ok(tokio::spawn(async { Ok(()) }))
            });

        let coordinator = IngestionCoordinator::new(registry, 100)?;
        coordinator.source_manager()
            .register_factory("mqtt".to_string(), Arc::new(factory))
            .await;

        coordinator.start().await?;

        // Verify source was spawned
    }

    #[tokio::test]
    async fn test_graceful_shutdown_stops_sources() {
        // Test shutdown sequence
    }
}
```

---

## 12. Future Enhancements

1. **Dead Letter Handler**: Background task to process validation failures
2. **Metrics Collection**: Prometheus metrics for throughput, errors, latency
3. **Dynamic Buffer Sizing**: Adjust channel capacity based on backpressure
4. **Source Health Dashboard**: Real-time status via API endpoint
5. **Config Hot Reload**: Apply config changes without restarting sources
6. **Multi-Coordinator**: Support for distributed coordination (Raft/consensus)

---

## Document Revision History

| Version | Date       | Author            | Changes                    |
|---------|------------|-------------------|----------------------------|
| 1.0.0   | 2025-12-16 | System Architect  | Initial design document    |

---

## References

- [AIR-005 SPARC Architecture](../../product/features/air-005/architecture/ARCHITECTURE.md)
- [StreamConfig Schema](../../core/src/types/stream_config.rs)
- [Source Trait](../../core/src/traits.rs)
- [IngestionRouter](../../apps/air-quality-app/src/coordinator/router.rs)
- [Tokio CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)
