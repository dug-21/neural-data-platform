# AIR-004 Multi-Stream Data Platform - Implementation Plan

**Document Information**
- Feature ID: AIR-004
- Version: 1.0.0
- Status: Planning Phase
- Created: 2025-12-15
- Author: PLANNER Agent
- Target: Raspberry Pi 5 (Ubuntu 25.04 ARM64)

---

## Executive Summary

AIR-004 extends the existing air-quality platform to support multiple heterogeneous data streams. This plan uses a **phased, additive approach** that preserves all working AIR-002/AIR-003 functionality while incrementally adding multi-stream capabilities.

**Critical Principle**: AIR-004 is an EXTENSION, not a replacement. The current MQTT ingestion pipeline MUST continue working throughout implementation.

**Total Estimated Timeline**: 15-20 days
**Lines of Code**: ~2,500 new (extends ~3,500 existing)
**Deployment Target**: Raspberry Pi 5 via `/workspaces/neural-data-platform/deploy/pi/`

---

## Implementation Strategy

### Core Principles

1. **Preserve Working Systems**: Zero breaking changes to existing MQTT pipeline
2. **Wrap, Don't Replace**: Extend existing components via wrappers and new modules
3. **London TDD**: Mock-driven development with clear interfaces
4. **Phase Gating**: Each phase has clear success criteria before proceeding
5. **Pi-First**: All changes validated on Raspberry Pi 5 ARM64 architecture

### Risk Mitigation

- **Baseline Testing**: Phase 0 establishes regression test suite
- **Parallel Deployment**: New coordinator runs alongside air-quality-app
- **Rollback Safety**: Bronze layer (Parquet) provides rollback point
- **Memory Constraints**: Monitor 896MB memory budget on Pi

---

## Phase 0: Verification and Protection (Day 1)

**Goal**: Document existing interfaces and establish baseline metrics

**Priority**: CRITICAL - Prevents breaking existing functionality

### Tasks

#### Task 0.1: Document Existing Interfaces
**Owner**: architect agent
**Estimated**: 2 hours
**Deliverable**: `/workspaces/neural-data-platform/product/features/air-004/architecture/EXISTING_INTERFACES.md`

**Actions**:
- Read and document `/workspaces/neural-data-platform/core/src/storage/parquet.rs`
  - Document `ParquetStore` public API
  - Document partition structure
  - Document WAL format
- Read and document TimeSeriesPoint structure location
- Read and document MqttSource interface from `neural-core`
- Read and document ConfigClient API from `/workspaces/neural-data-platform/config-client/`

**Acceptance Criteria**:
- All public interfaces documented with signatures
- Data structures serialization formats captured
- File locations and module paths recorded

#### Task 0.2: Create Integration Test Baseline
**Owner**: tester agent
**Estimated**: 4 hours
**Deliverable**: `/workspaces/neural-data-platform/apps/air-quality-app/tests/air002_baseline_test.rs`

**Actions**:
- Create integration test suite for current MQTT pipeline
  - Test MQTT ingestion end-to-end
  - Test config loading hierarchy (etcd > env > yaml > defaults)
  - Test WAL replay functionality
  - Test Parquet write and query
- Run existing test suite and capture output
  - `cargo test --package air-quality-app`
  - `cargo test --package config-client`
  - Document test coverage baseline

**Acceptance Criteria**:
- All existing tests pass (100% baseline)
- New baseline test covers MQTT -> Parquet pipeline
- Test can be re-run to detect regressions

#### Task 0.3: Establish Performance Baseline
**Owner**: perf-analyzer agent
**Estimated**: 2 hours
**Deliverable**: `/workspaces/neural-data-platform/product/features/air-004/BASELINE_METRICS.md`

**Actions**:
- Benchmark current performance metrics:
  - Config read latency from etcd
  - MQTT ingestion rate (messages/sec)
  - Storage write throughput (records/sec)
  - Memory usage (air-quality-app container)
  - API response times (GET /api/v1/readings)
- Document current Parquet file sizes and compression ratios
- Capture Docker stats output after 1 hour of operation

**Acceptance Criteria**:
- Baseline metrics documented with measurement method
- Performance targets defined (no regression > 10%)

### Phase 0 Success Criteria

- [ ] All existing interfaces documented
- [ ] Regression test suite created and passing
- [ ] Performance baseline established
- [ ] Current air-quality deployment verified working
- [ ] Phase 0 review completed with stakeholders

**Dependencies**: None
**Blockers**: None
**Go/No-Go Decision**: Required before Phase 1

---

## Phase 1: Foundation (Days 2-4)

**Goal**: Add stream registry and core data types WITHOUT breaking existing systems

**Priority**: HIGH - Foundation for all subsequent work

### Tasks

#### Task 1.1: Extend TimeSeriesPoint for Multi-Stream
**Owner**: backend-dev agent
**Estimated**: 4 hours
**Deliverable**: Extension to existing TimeSeriesPoint type

**Actions**:
- Locate current TimeSeriesPoint definition (likely in `core/src/types.rs` or similar)
- Add optional `stream_id` field with `#[serde(default = "default_stream_id")]`
- Add backward compatibility test (deserialize old Parquet files)
- Ensure serialization format unchanged for existing fields

**Acceptance Criteria**:
- Existing Parquet files can be read without errors
- New field defaults to "air-quality" for backward compat
- All existing tests still pass
- London TDD: Mock ParquetStore tests verify compatibility

**File Location**: Extend existing file (TBD based on Task 0.1)

**Backward Compatibility**:
```rust
#[derive(Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
    #[serde(default = "default_stream_id")]
    pub stream_id: Option<String>,  // NEW
}

fn default_stream_id() -> Option<String> {
    Some("air-quality".to_string())
}
```

#### Task 1.2: Create StreamRecord Wrapper Type
**Owner**: backend-dev agent
**Estimated**: 3 hours
**Deliverable**: `/workspaces/neural-data-platform/core/src/types/stream_record.rs`

**Actions**:
- Create StreamRecord struct that wraps TimeSeriesPoint
- Implement `From<TimeSeriesPoint>` for backward compatibility
- Add RecordMetadata struct for source tracking
- Write unit tests for conversions

**Acceptance Criteria**:
- StreamRecord wraps TimeSeriesPoint without duplication
- Can convert TimeSeriesPoint -> StreamRecord losslessly
- London TDD: All conversions have mock-based tests

**Implementation**:
```rust
pub struct StreamRecord {
    pub stream_id: String,
    pub point: TimeSeriesPoint,
    pub metadata: Option<RecordMetadata>,
}

pub struct RecordMetadata {
    pub source_id: String,
    pub ingestion_time: DateTime<Utc>,
}

impl From<TimeSeriesPoint> for StreamRecord {
    fn from(point: TimeSeriesPoint) -> Self {
        Self {
            stream_id: point.stream_id.unwrap_or_else(|| "air-quality".to_string()),
            point,
            metadata: None,
        }
    }
}
```

#### Task 1.3: Create StreamConfig Type
**Owner**: backend-dev agent
**Estimated**: 4 hours
**Deliverable**: `/workspaces/neural-data-platform/core/src/types/stream_config.rs`

**Actions**:
- Define StreamConfig struct (stream metadata)
- Define Schema struct (field definitions)
- Define SchemaField struct (name, type, unit, nullable)
- Define SourceConfig enum (MQTT, HTTP, Webhook variants)
- Add serde serialization for etcd storage

**Acceptance Criteria**:
- Types serialize to/from JSON (etcd format)
- Schema validation logic implemented
- Unit tests for all type conversions
- London TDD: Mock etcd client tests

**Implementation**:
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StreamConfig {
    pub id: String,
    pub description: String,
    pub retention_days: u32,
    pub compression_after_days: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Schema {
    pub version: u32,
    pub fields: Vec<SchemaField>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SchemaField {
    pub name: String,
    pub field_type: FieldType,
    pub unit: Option<String>,
    pub nullable: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FieldType {
    Int,
    Float,
    String,
    Boolean,
    Json,
    Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SourceConfig {
    Mqtt(MqttSourceConfig),
    HttpPoll(HttpPollSourceConfig),
    Webhook(WebhookSourceConfig),
}
```

#### Task 1.4: Extend ConfigClient for Stream Registry
**Owner**: backend-dev agent
**Estimated**: 6 hours
**Deliverable**: `/workspaces/neural-data-platform/config-client/src/stream_registry.rs`

**Actions**:
- Create StreamRegistry struct that wraps ConfigClient
- Implement `load_stream(stream_id)` using ConfigClient::get()
- Implement `list_streams()` using ConfigClient::list()
- Implement `watch_streams()` using ConfigClient::watch()
- Reuse config-client patterns (get_with_env, error handling)
- Write integration tests with test etcd instance

**Acceptance Criteria**:
- StreamRegistry uses existing ConfigClient internally
- No changes to existing ConfigClient API
- Watch mechanism triggers callbacks on stream updates
- London TDD: Mock ConfigClient for unit tests
- Integration tests with real etcd (testcontainers)

**Implementation**:
```rust
pub struct StreamRegistry {
    client: ConfigClient,
}

impl StreamRegistry {
    pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError> {
        let client = ConfigClient::with_prefix(endpoints, "/streams").await?;
        Ok(Self { client })
    }

    pub async fn load_stream(&self, stream_id: &str) -> Result<FullStreamConfig, ConfigError> {
        let config = self.client.get::<StreamConfig>(&format!("/{}/config", stream_id)).await?;
        let schema = self.client.get::<Schema>(&format!("/{}/schema", stream_id)).await?;
        let sources = self.client.get::<Vec<SourceConfig>>(&format!("/{}/sources", stream_id)).await?;

        Ok(FullStreamConfig {
            id: stream_id.to_string(),
            config,
            schema,
            sources,
        })
    }

    pub async fn list_streams(&self) -> Result<Vec<String>, ConfigError> {
        let keys = self.client.list("/").await?;
        // Extract stream IDs from keys like "/air-quality/config"
        Ok(keys.into_iter()
            .filter_map(|k| k.split('/').nth(1).map(String::from))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect())
    }

    pub async fn watch_streams<F>(&self, callback: F) -> Result<WatchHandle, ConfigError>
    where
        F: Fn(String, StreamEvent) + Send + Sync + 'static,
    {
        self.client.watch("/", move |key, value| {
            // Parse key to extract stream_id and component
            // Trigger callback with parsed event
        }).await
    }
}
```

### Phase 1 Success Criteria

- [ ] TimeSeriesPoint extended with backward compatibility verified
- [ ] StreamRecord type created with conversion tests
- [ ] StreamConfig types defined and serialization tested
- [ ] StreamRegistry implemented and integration tested
- [ ] All Phase 0 baseline tests still passing
- [ ] No impact on running air-quality-app deployment

**Dependencies**: Phase 0 complete
**Blockers**: None
**Go/No-Go Decision**: Required before Phase 2

---

## Phase 2: Storage Layer Extensions (Days 5-8)

**Goal**: Extend storage to support multiple streams while preserving single-stream functionality

**Priority**: HIGH - Data durability risk

### Tasks

#### Task 2.1: Extend ParquetStore for Multi-Stream
**Owner**: backend-dev agent
**Estimated**: 8 hours
**Deliverable**: Extension to `/workspaces/neural-data-platform/core/src/storage/parquet.rs`

**Actions**:
- Add `write_batch_for_stream(stream_id, points)` method
- Implement stream-based partitioning:
  - New path: `{base}/streams/{stream_id}/data/{location}/year=.../`
  - Old path still works: `{base}/data/{location}/year=.../` (maps to "air-quality")
- Keep existing `write_batch()` unchanged (backward compat)
- Update WAL to include stream_id
- Test with existing air-quality Parquet files

**Acceptance Criteria**:
- Existing `write_batch()` method unchanged and working
- New `write_batch_for_stream()` writes to separate partitions
- WAL format backward compatible
- Can read old Parquet files without stream_id
- London TDD: Mock file system operations
- Integration tests verify partition structure

**Preservation Notes**:
- DO NOT modify existing write_batch signature
- DO NOT change Parquet schema for existing files
- Ensure WAL replay handles both old and new formats

#### Task 2.2: Create MultiStreamStore Wrapper
**Owner**: backend-dev agent
**Estimated**: 6 hours
**Deliverable**: `/workspaces/neural-data-platform/core/src/storage/multi_stream_store.rs`

**Actions**:
- Create MultiStreamStore struct with HashMap of ParquetStore instances
- Implement lazy loading (get_or_create_store)
- Delegate to individual ParquetStore instances
- Handle stream-specific base paths

**Acceptance Criteria**:
- Each stream gets isolated ParquetStore
- Lazy initialization on first write
- WAL replay called once per stream on startup
- London TDD: Mock ParquetStore creation

**Implementation**:
```rust
pub struct MultiStreamStore {
    base_path: PathBuf,
    stores: Arc<RwLock<HashMap<String, Arc<ParquetStore>>>>,
}

impl MultiStreamStore {
    pub async fn get_or_create_store(&self, stream_id: &str) -> Result<Arc<ParquetStore>, Error> {
        // Check cache
        {
            let stores = self.stores.read().await;
            if let Some(store) = stores.get(stream_id) {
                return Ok(Arc::clone(store));
            }
        }

        // Create new store
        let stream_path = self.base_path.join("streams").join(stream_id);
        let store = Arc::new(ParquetStore::new(stream_path)?);
        store.replay_wal().await?;

        // Cache it
        self.stores.write().await.insert(stream_id.to_string(), Arc::clone(&store));
        Ok(store)
    }

    pub async fn write_batch(&self, stream_id: &str, points: Vec<TimeSeriesPoint>) -> Result<(), Error> {
        let store = self.get_or_create_store(stream_id).await?;
        store.write_batch(points).await
    }
}
```

#### Task 2.3: Implement TimescaleDB Adapter (Optional for Pi)
**Owner**: backend-dev agent
**Estimated**: 10 hours
**Deliverable**: `/workspaces/neural-data-platform/core/src/storage/timescale.rs`

**Actions**:
- Create TimescaleAdapter struct with sqlx PgPool
- Implement `ensure_table(stream_config)` - dynamic DDL generation
- Implement `write_batch(stream_id, points)` - INSERT statements
- Add connection pooling and retry logic
- Copy batching patterns from StorageWriter

**Acceptance Criteria**:
- Can create hypertable from StreamConfig schema
- Batched writes with configurable size
- Connection retry with exponential backoff
- London TDD: Mock PgPool for unit tests
- Integration tests with testcontainers postgres

**Note**: TimescaleDB NOT deployed on Pi initially. This enables Silver layer for future deployment.

**Implementation**:
```rust
pub struct TimescaleAdapter {
    pool: PgPool,
}

impl TimescaleAdapter {
    pub async fn new(connection_string: &str) -> Result<Self, Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(connection_string)
            .await?;
        Ok(Self { pool })
    }

    pub async fn ensure_table(&self, stream_config: &FullStreamConfig) -> Result<(), Error> {
        let ddl = self.generate_ddl(stream_config)?;
        sqlx::query(&ddl).execute(&self.pool).await?;

        // Create hypertable
        let create_hyper = format!(
            "SELECT create_hypertable('{}', 'timestamp', if_not_exists => TRUE, chunk_time_interval => INTERVAL '1 day')",
            stream_config.id
        );
        sqlx::query(&create_hyper).execute(&self.pool).await?;

        Ok(())
    }

    pub async fn write_batch(&self, stream_id: &str, points: Vec<TimeSeriesPoint>) -> Result<(), Error> {
        // Build parameterized INSERT query
        // Execute with sqlx transaction
        // Handle conflicts (on conflict do nothing)
    }

    fn generate_ddl(&self, stream_config: &FullStreamConfig) -> Result<String, Error> {
        // Convert Schema fields to PostgreSQL column definitions
    }
}
```

#### Task 2.4: Create Storage Layer Manager (Dual-Write Coordinator)
**Owner**: backend-dev agent
**Estimated**: 8 hours
**Deliverable**: `/workspaces/neural-data-platform/apps/air-quality-app/src/storage/layer_manager.rs`

**Actions**:
- Create StorageLayerManager that coordinates Bronze + Silver writes
- Implement dual-write with Bronze-first pattern
- Add retry queue for Silver write failures
- Copy batching logic from StorageWriter
- Add circuit breaker for TimescaleDB failures

**Acceptance Criteria**:
- Bronze write always succeeds (or errors immediately)
- Silver write failures don't block Bronze
- Retry queue persists across restarts
- London TDD: Mock both stores
- Integration tests verify failure scenarios

**Implementation**:
```rust
pub struct StorageLayerManager {
    bronze_store: Arc<MultiStreamStore>,
    silver_adapter: Option<Arc<TimescaleAdapter>>,
    retry_queue: Arc<Mutex<VecDeque<RetryItem>>>,
}

impl StorageLayerManager {
    pub async fn write_batch(&self, stream_id: &str, points: Vec<TimeSeriesPoint>) -> Result<(), Error> {
        // 1. Write to Bronze (MUST succeed)
        self.bronze_store.write_batch(stream_id, points.clone()).await?;

        // 2. Write to Silver (best-effort)
        if let Some(silver) = &self.silver_adapter {
            match silver.write_batch(stream_id, points.clone()).await {
                Ok(_) => {
                    metrics::increment("silver_writes_success", vec![("stream", stream_id)]);
                }
                Err(e) => {
                    warn!("Silver write failed, queuing for retry: {}", e);
                    self.retry_queue.lock().await.push_back(RetryItem {
                        stream_id: stream_id.to_string(),
                        points,
                        attempts: 0,
                        next_retry: Instant::now() + Duration::from_secs(1),
                    });
                    metrics::increment("silver_writes_failed", vec![("stream", stream_id)]);
                }
            }
        }

        Ok(())
    }

    async fn process_retry_queue(&self) {
        // Background task: retry failed Silver writes with exponential backoff
    }
}
```

### Phase 2 Success Criteria

- [ ] ParquetStore extended with multi-stream partitioning
- [ ] MultiStreamStore wrapper created and tested
- [ ] TimescaleDB adapter implemented (unit tested, not deployed)
- [ ] StorageLayerManager coordinates dual-writes
- [ ] Existing air-quality writes still working
- [ ] All Phase 0 baseline tests still passing
- [ ] No Parquet file corruption or data loss

**Dependencies**: Phase 1 complete
**Blockers**: None (TimescaleDB deployment optional)
**Go/No-Go Decision**: Required before Phase 3

---

## Phase 3: Source Implementations (Days 9-12)

**Goal**: Add HTTP polling and webhook sources following MqttHandler pattern

**Priority**: MEDIUM - Enables multi-source ingestion

### Tasks

#### Task 3.1: Verify MqttSource Interface (Documentation Only)
**Owner**: architect agent
**Estimated**: 2 hours
**Deliverable**: Update to `/workspaces/neural-data-platform/product/features/air-004/architecture/EXISTING_INTERFACES.md`

**Actions**:
- Document MqttSource public interface from `neural-core`
- Document MqttHandler usage pattern from AIR-002
- Confirm no changes needed to existing MQTT code
- Document integration points for coordinator

**Acceptance Criteria**:
- MqttSource API fully documented
- MqttHandler pattern documented (channel-based forwarding)
- Confirmed: NO CHANGES to existing MQTT components

#### Task 3.2: Create MqttSource Wrapper for Coordinator
**Owner**: backend-dev agent
**Estimated**: 4 hours
**Deliverable**: `/workspaces/neural-data-platform/apps/air-quality-app/src/sources/mqtt_wrapper.rs`

**Actions**:
- Create MqttSourceWrapper that uses existing MqttHandler
- Adapt SourceConfig (MQTT variant) to MqttConfig conversion
- Implement channel creation and task spawning
- Reuse ALL existing MqttHandler logic

**Acceptance Criteria**:
- Uses existing MqttHandler without modifications
- Converts generic SourceConfig -> MqttConfig
- Returns mpsc::Receiver for coordinator
- London TDD: Mock MqttHandler creation

**Implementation**:
```rust
pub struct MqttSourceWrapper {
    stream_id: String,
    config: MqttSourceConfig,
}

impl MqttSourceWrapper {
    pub async fn spawn(stream_id: String, config: MqttSourceConfig) -> Result<mpsc::Receiver<TimeSeriesPoint>, Error> {
        // Convert to MqttConfig (AIR-002 format)
        let mqtt_config = MqttConfig {
            broker_url: config.broker_url,
            port: config.port,
            // ... other fields
        };

        // Create channel
        let (tx, rx) = mpsc::channel(config.buffer_capacity);

        // Spawn MqttHandler (existing code)
        let handler = MqttHandler::new(mqtt_config, tx).await?;
        tokio::spawn(async move {
            if let Err(e) = handler.run().await {
                error!("MQTT handler error: {}", e);
            }
        });

        Ok(rx)
    }
}
```

#### Task 3.3: Implement HttpPoller Source
**Owner**: backend-dev agent
**Estimated**: 10 hours
**Deliverable**: `/workspaces/neural-data-platform/apps/air-quality-app/src/sources/http_poller.rs`

**Actions**:
- Create HttpPoller struct following MqttHandler pattern
- Implement polling loop with tokio::interval
- Parse JSON/CSV responses to TimeSeriesPoint
- Add authentication support (Bearer, API key, Basic)
- Add retry logic with exponential backoff
- Handle HTTP errors gracefully

**Acceptance Criteria**:
- Follows MqttHandler channel pattern
- Polls at configurable interval (min 10 seconds)
- Supports GET and POST methods
- Handles JSON and CSV response formats
- London TDD: Mock HTTP server (wiremock)
- Integration tests with real HTTP endpoints

**Implementation**:
```rust
pub struct HttpPoller {
    stream_id: String,
    config: HttpPollSourceConfig,
    client: reqwest::Client,
    sender: mpsc::Sender<TimeSeriesPoint>,
}

impl HttpPoller {
    pub async fn run(self) -> Result<(), Error> {
        let mut interval = tokio::time::interval(self.config.interval);

        loop {
            interval.tick().await;

            match self.poll_once().await {
                Ok(points) => {
                    for point in points {
                        if let Err(e) = self.sender.send(point).await {
                            error!("Failed to send point: {}", e);
                        }
                    }
                    metrics::increment("http_poll_success", vec![("stream", &self.stream_id)]);
                }
                Err(e) => {
                    error!("HTTP poll failed: {}", e);
                    metrics::increment("http_poll_errors", vec![("stream", &self.stream_id)]);
                }
            }
        }
    }

    async fn poll_once(&self) -> Result<Vec<TimeSeriesPoint>, Error> {
        let response = self.client
            .request(self.config.method.clone(), &self.config.url)
            .headers(self.config.headers.clone())
            .timeout(self.config.timeout)
            .send()
            .await?;

        let body = response.text().await?;
        self.parse_response(&body)
    }

    fn parse_response(&self, body: &str) -> Result<Vec<TimeSeriesPoint>, Error> {
        match self.config.response_format {
            ResponseFormat::Json => self.parse_json(body),
            ResponseFormat::Csv => self.parse_csv(body),
        }
    }
}
```

#### Task 3.4: Implement WebhookHandler Source
**Owner**: backend-dev agent
**Estimated**: 10 hours
**Deliverable**: `/workspaces/neural-data-platform/apps/air-quality-app/src/sources/webhook_handler.rs`

**Actions**:
- Create WebhookHandler using Axum (already in project)
- Implement POST endpoint `/api/streams/{stream_id}/events`
- Add authentication middleware (Bearer token)
- Parse request body to TimeSeriesPoint
- Forward to channel (same pattern as MqttHandler)
- Add rate limiting (1000 req/min per IP)

**Acceptance Criteria**:
- Axum server listens on configurable port
- Validates authentication before processing
- Responds with 202 Accepted immediately
- Forwards to channel asynchronously
- London TDD: Mock Axum requests
- Integration tests with real HTTP clients

**Implementation**:
```rust
pub struct WebhookHandler {
    stream_id: String,
    config: WebhookSourceConfig,
    sender: mpsc::Sender<TimeSeriesPoint>,
}

impl WebhookHandler {
    pub async fn run(self) -> Result<(), Error> {
        let app = Router::new()
            .route("/events", post(handle_event))
            .layer(middleware::from_fn(auth_middleware))
            .with_state(Arc::new(WebhookState {
                stream_id: self.stream_id.clone(),
                sender: self.sender,
            }));

        let addr = format!("0.0.0.0:{}", self.config.port);
        axum::Server::bind(&addr.parse()?)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }
}

async fn handle_event(
    State(state): State<Arc<WebhookState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<StatusCode, Error> {
    // Parse payload to TimeSeriesPoint
    let point = parse_webhook_payload(payload)?;

    // Forward to channel (non-blocking)
    state.sender.try_send(point)?;

    Ok(StatusCode::ACCEPTED)
}
```

#### Task 3.5: Implement Source Health Checks
**Owner**: backend-dev agent
**Estimated**: 4 hours
**Deliverable**: Extend source implementations with health check methods

**Actions**:
- Add `health_check()` to each source type
- MQTT: delegate to existing MqttHandler health check
- HTTP: HEAD request to endpoint
- Webhook: check if server is running
- Return standardized HealthStatus struct

**Acceptance Criteria**:
- All sources implement health_check()
- Health checks complete within 5 seconds
- Return actionable error messages
- London TDD: Mock health check responses

### Phase 3 Success Criteria

- [ ] MqttSource wrapper created (reuses existing code)
- [ ] HttpPoller implemented and tested
- [ ] WebhookHandler implemented and tested
- [ ] Health checks implemented for all source types
- [ ] All sources follow MqttHandler channel pattern
- [ ] Integration tests passing for all sources
- [ ] Existing MQTT ingestion still working

**Dependencies**: Phase 1 complete (StreamConfig types)
**Blockers**: None
**Go/No-Go Decision**: Required before Phase 4

---

## Phase 4: Coordination Layer (Days 13-17)

**Goal**: Orchestrate multiple sources and route to storage layers

**Priority**: CRITICAL PATH - Ties everything together

### Tasks

#### Task 4.1: Implement Ingestion Router
**Owner**: backend-dev agent
**Estimated**: 8 hours
**Deliverable**: `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/router.rs`

**Actions**:
- Create IngestionRouter that validates and routes points
- Implement schema validation per stream
- Add stream_id tagging to TimeSeriesPoint
- Forward validated points to storage channel
- Route invalid points to dead-letter queue

**Acceptance Criteria**:
- Validates against stream schema
- Routes to correct storage writer channel
- Logs validation errors with details
- London TDD: Mock schema validator
- Integration tests with valid/invalid points

**Implementation**:
```rust
pub struct IngestionRouter {
    registry: Arc<StreamRegistry>,
    storage_channels: Arc<RwLock<HashMap<String, mpsc::Sender<TimeSeriesPoint>>>>,
    dead_letter_tx: mpsc::Sender<DeadLetterItem>,
}

impl IngestionRouter {
    pub async fn route_point(&self, source_id: &str, stream_id: &str, point: TimeSeriesPoint) -> Result<(), Error> {
        // 1. Get schema for stream
        let stream_config = self.registry.load_stream(stream_id).await?;

        // 2. Validate point against schema
        if let Err(e) = self.validate_point(&point, &stream_config.schema) {
            warn!("Validation failed for stream {}: {}", stream_id, e);
            self.dead_letter_tx.send(DeadLetterItem {
                stream_id: stream_id.to_string(),
                source_id: source_id.to_string(),
                point,
                error: e.to_string(),
            }).await?;
            return Ok(());
        }

        // 3. Enrich with metadata
        let mut enriched = point;
        enriched.tags.insert("stream_id".to_string(), stream_id.to_string());
        enriched.tags.insert("source_id".to_string(), source_id.to_string());

        // 4. Route to storage writer
        let channels = self.storage_channels.read().await;
        if let Some(tx) = channels.get(stream_id) {
            tx.send(enriched).await?;
            metrics::increment("points_routed", vec![("stream", stream_id)]);
        } else {
            error!("No storage writer for stream: {}", stream_id);
        }

        Ok(())
    }

    fn validate_point(&self, point: &TimeSeriesPoint, schema: &Schema) -> Result<(), ValidationError> {
        // Validate each tag against schema fields
        // Check types, ranges, nullability
    }
}
```

#### Task 4.2: Implement Source Manager
**Owner**: backend-dev agent
**Estimated**: 10 hours
**Deliverable**: `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`

**Actions**:
- Create SourceManager that spawns and manages sources
- Implement dynamic source spawning from SourceConfig
- Create wrapper for each source type (MQTT, HTTP, Webhook)
- Maintain source handle registry (for health checks)
- Implement graceful shutdown per source

**Acceptance Criteria**:
- Can spawn all source types dynamically
- Each source runs in isolated tokio task
- Health checks work for all active sources
- Graceful shutdown closes all sources cleanly
- London TDD: Mock source spawning

**Implementation**:
```rust
pub struct SourceManager {
    active_sources: Arc<RwLock<HashMap<String, SourceHandle>>>,
}

pub struct SourceHandle {
    source_id: String,
    stream_id: String,
    source_type: SourceType,
    task_handle: JoinHandle<()>,
    shutdown_tx: oneshot::Sender<()>,
}

impl SourceManager {
    pub async fn spawn_source(&self, stream_id: String, source_config: SourceConfig) -> Result<String, Error> {
        let source_id = format!("{}-{}", stream_id, source_config.id());
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let task_handle = match source_config {
            SourceConfig::Mqtt(mqtt_config) => {
                let receiver = MqttSourceWrapper::spawn(stream_id.clone(), mqtt_config).await?;
                tokio::spawn(async move {
                    // Forward from receiver to router until shutdown
                })
            }
            SourceConfig::HttpPoll(http_config) => {
                let poller = HttpPoller::new(stream_id.clone(), http_config);
                tokio::spawn(async move {
                    poller.run().await
                })
            }
            SourceConfig::Webhook(webhook_config) => {
                let handler = WebhookHandler::new(stream_id.clone(), webhook_config);
                tokio::spawn(async move {
                    handler.run().await
                })
            }
        };

        let handle = SourceHandle {
            source_id: source_id.clone(),
            stream_id,
            source_type: source_config.source_type(),
            task_handle,
            shutdown_tx,
        };

        self.active_sources.write().await.insert(source_id.clone(), handle);
        Ok(source_id)
    }

    pub async fn shutdown_source(&self, source_id: &str) -> Result<(), Error> {
        // Send shutdown signal and await task completion
    }

    pub async fn health_check(&self, source_id: &str) -> Result<HealthStatus, Error> {
        // Delegate to source-specific health check
    }
}
```

#### Task 4.3: Extend StorageWriter for Multi-Stream
**Owner**: backend-dev agent
**Estimated**: 6 hours
**Deliverable**: Extension to `/workspaces/neural-data-platform/apps/air-quality-app/src/pipeline/storage_writer.rs`

**Actions**:
- Update StorageWriter to use StorageLayerManager
- Keep existing batching logic (100 points / 5s timeout)
- Add stream_id awareness (for metrics)
- Maintain backward compatibility with single-stream use

**Acceptance Criteria**:
- StorageWriter delegates to StorageLayerManager
- Batching logic unchanged
- Existing air-quality-app usage still works
- London TDD: Mock StorageLayerManager

**Modification**:
```rust
// Minimal change to existing StorageWriter
impl StorageWriter {
    pub fn new(
        layer_manager: Arc<StorageLayerManager>,  // Changed from ParquetStore
        stream_id: String,                        // Added
        receiver: mpsc::Receiver<TimeSeriesPoint>,
        batch_size: Option<usize>,
        batch_timeout: Option<Duration>,
    ) -> Self {
        // ... rest unchanged
    }

    pub async fn run(mut self) -> Result<(), Error> {
        // ... batching logic unchanged

        // Only change: delegate to layer_manager
        self.layer_manager.write_batch(&self.stream_id, buffer.clone()).await?;
    }
}
```

#### Task 4.4: Implement Ingestion Coordinator
**Owner**: backend-dev agent
**Estimated**: 12 hours
**Deliverable**: `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/ingestion_coordinator.rs`

**Actions**:
- Create IngestionCoordinator that orchestrates all components
- Initialize StreamRegistry from etcd
- Spawn SourceManager and IngestionRouter
- Spawn StorageWriter per stream
- Handle registry watch events (add/remove/update streams)
- Implement graceful shutdown of all components

**Acceptance Criteria**:
- Loads all streams from registry on startup
- Spawns sources and writers for each stream
- Handles dynamic stream addition via watch
- Graceful shutdown completes all in-flight writes
- London TDD: Mock all subcomponents
- Integration test with full pipeline

**Implementation**:
```rust
pub struct IngestionCoordinator {
    registry: Arc<StreamRegistry>,
    source_manager: Arc<SourceManager>,
    router: Arc<IngestionRouter>,
    storage_writers: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
    layer_manager: Arc<StorageLayerManager>,
}

impl IngestionCoordinator {
    pub async fn new(etcd_endpoints: &[&str], storage_config: StorageConfig) -> Result<Self, Error> {
        // 1. Initialize registry
        let registry = Arc::new(StreamRegistry::new(etcd_endpoints).await?);

        // 2. Initialize storage
        let bronze_store = Arc::new(MultiStreamStore::new(storage_config.base_path)?);
        let silver_adapter = if storage_config.timescale_enabled {
            Some(Arc::new(TimescaleAdapter::new(&storage_config.timescale_url).await?))
        } else {
            None
        };
        let layer_manager = Arc::new(StorageLayerManager::new(bronze_store, silver_adapter));

        // 3. Initialize router
        let router = Arc::new(IngestionRouter::new(Arc::clone(&registry)));

        // 4. Initialize source manager
        let source_manager = Arc::new(SourceManager::new());

        // 5. Load existing streams and spawn components
        let streams = registry.list_streams().await?;
        let mut storage_writers = HashMap::new();

        for stream_id in streams {
            let stream_config = registry.load_stream(&stream_id).await?;

            // Spawn storage writer for this stream
            let writer_handle = Self::spawn_storage_writer(
                stream_id.clone(),
                Arc::clone(&layer_manager),
            ).await?;
            storage_writers.insert(stream_id.clone(), writer_handle);

            // Spawn all sources for this stream
            for source_config in stream_config.sources {
                source_manager.spawn_source(stream_id.clone(), source_config).await?;
            }
        }

        Ok(Self {
            registry,
            source_manager,
            router,
            storage_writers: Arc::new(RwLock::new(storage_writers)),
            layer_manager,
        })
    }

    async fn spawn_storage_writer(stream_id: String, layer_manager: Arc<StorageLayerManager>) -> Result<JoinHandle<()>, Error> {
        let (tx, rx) = mpsc::channel(1000);

        // Store tx in router for this stream
        // (router needs access to send points)

        let writer = StorageWriter::new(
            Arc::clone(&layer_manager),
            stream_id.clone(),
            rx,
            Some(100),  // batch_size
            Some(Duration::from_secs(5)),  // batch_timeout
        );

        Ok(tokio::spawn(async move {
            if let Err(e) = writer.run().await {
                error!("Storage writer error for {}: {}", stream_id, e);
            }
        }))
    }

    pub async fn run(self) -> Result<(), Error> {
        // Watch for registry changes
        let registry = Arc::clone(&self.registry);
        registry.watch_streams(|stream_id, event| {
            match event {
                StreamEvent::Added(config) => {
                    // Spawn new sources and writer
                }
                StreamEvent::Updated(old, new) => {
                    // Gracefully restart sources
                }
                StreamEvent::Deleted(_) => {
                    // Shutdown sources and writer
                }
            }
        }).await?;

        // Run until shutdown signal
        tokio::signal::ctrl_c().await?;
        self.shutdown().await
    }

    async fn shutdown(self) -> Result<(), Error> {
        // 1. Stop accepting new data (shutdown sources)
        // 2. Flush all storage writers
        // 3. Close registry watch
    }
}
```

#### Task 4.5: Integration Testing
**Owner**: tester agent
**Estimated**: 8 hours
**Deliverable**: `/workspaces/neural-data-platform/apps/air-quality-app/tests/coordinator_integration_test.rs`

**Actions**:
- Create end-to-end integration test with test etcd
- Test multi-stream scenario (2+ streams)
- Test dynamic stream addition via etcd PUT
- Test source failure and recovery
- Test storage layer writes (verify Parquet files)
- Run baseline regression tests

**Acceptance Criteria**:
- Integration test covers full pipeline (source -> storage)
- Tests pass with real etcd (testcontainers)
- All Phase 0 baseline tests still passing
- No regressions in existing air-quality functionality

### Phase 4 Success Criteria

- [ ] IngestionRouter validates and routes points correctly
- [ ] SourceManager spawns and manages all source types
- [ ] StorageWriter extended for multi-stream (backward compatible)
- [ ] IngestionCoordinator orchestrates all components
- [ ] Integration tests passing for 2+ streams
- [ ] Dynamic stream addition works via etcd watch
- [ ] Existing air-quality-app deployment unaffected
- [ ] All Phase 0 baseline tests passing

**Dependencies**: Phase 2 (Storage), Phase 3 (Sources) complete
**Blockers**: None
**Go/No-Go Decision**: Required before Phase 5

---

## Phase 5: Deployment and Integration (Days 18-20)

**Goal**: Deploy to Raspberry Pi 5 and validate production operation

**Priority**: CRITICAL - Production deployment

### Tasks

#### Task 5.1: Update Pi Docker Compose Configuration
**Owner**: cicd-engineer agent
**Estimated**: 4 hours
**Deliverable**: Extension to `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`

**Actions**:
- Add TimescaleDB service (optional, for Silver layer testing)
- Update air-quality-app environment variables
- Configure memory limits (896MB total budget)
- Add health checks for all services
- Update volume mounts for multi-stream storage

**Acceptance Criteria**:
- Docker Compose starts all services successfully
- Health checks pass within 5 minutes
- Memory usage within budget (<896MB)
- Existing volumes preserved (pi_air-quality-data, pi_etcd-data)

**Configuration**:
```yaml
# Extension to deploy/pi/docker-compose.yml
services:
  air-quality-app:
    environment:
      - ENABLE_MULTI_STREAM=true  # Feature flag for gradual rollout
      - STORAGE_MULTI_STREAM_PATH=/app/data/streams
      - TIMESCALE_ENABLED=false   # Disabled initially on Pi
    deploy:
      resources:
        limits:
          memory: 896M
    volumes:
      - pi_air-quality-data:/app/data
      - pi_etcd-data:/etcd-data (shared with etcd)

  # Optional: TimescaleDB (separate deployment, not on Pi initially)
  # timescaledb:
  #   image: timescale/timescaledb:latest-pg15
  #   ...
```

#### Task 5.2: Populate Initial Stream Configurations in etcd
**Owner**: cicd-engineer agent
**Estimated**: 2 hours
**Deliverable**: `/workspaces/neural-data-platform/deploy/pi/configs/streams/`

**Actions**:
- Create stream config for air-quality (migration from AIR-002)
- Create example stream config for weather (HTTP polling demo)
- Create example stream config for home-events (webhook demo)
- Write shell script to populate etcd on deploy

**Acceptance Criteria**:
- air-quality stream config matches existing behavior
- Example streams are optional (can be disabled)
- Script can be run idempotently

**Stream Configs**:
```bash
# deploy/pi/configs/streams/air-quality.sh
#!/bin/bash
etcdctl put /streams/air-quality/config '{
  "id": "air-quality",
  "description": "AirGradient sensor data",
  "retention_days": 365,
  "compression_after_days": 7
}'

etcdctl put /streams/air-quality/schema '{
  "version": 1,
  "fields": [
    {"name": "pm25", "field_type": "Float", "nullable": false, "unit": "µg/m³"},
    {"name": "pm10", "field_type": "Float", "nullable": true, "unit": "µg/m³"},
    {"name": "co2", "field_type": "Int", "nullable": false, "unit": "ppm"},
    {"name": "temperature", "field_type": "Float", "nullable": true, "unit": "°C"},
    {"name": "humidity", "field_type": "Float", "nullable": true, "unit": "%"}
  ]
}'

etcdctl put /streams/air-quality/sources '[
  {
    "type": "mqtt",
    "id": "airgradient-mqtt",
    "params": {
      "broker_url": "mosquitto",
      "port": 1883,
      "topic": "airgradient/readings/+",
      "qos": 1,
      "buffer_capacity": 1000
    }
  }
]'
```

#### Task 5.3: Build and Deploy to Raspberry Pi 5
**Owner**: cicd-engineer agent
**Estimated**: 6 hours
**Deliverable**: Successful Pi deployment

**Actions**:
- Update Dockerfile for ARM64 build
- Cross-compile or build on Pi (expect 15-30 min build time)
- Run `./deploy/pi/deploy.sh build`
- Run `./deploy/pi/deploy.sh start`
- Run stream config population scripts
- Verify services start and health checks pass

**Acceptance Criteria**:
- Build completes within 30 minutes
- All services start successfully
- Health checks green within 5 minutes
- MQTT ingestion resumes (air-quality stream)
- Memory usage <896MB after 1 hour

**Deployment Steps**:
```bash
# On Raspberry Pi 5
cd /workspaces/neural-data-platform/deploy/pi

# Build Docker images (ARM64 cross-compile or local build)
./deploy.sh build

# Start services
./deploy.sh start

# Populate stream configs
./configs/streams/air-quality.sh

# Verify status
./deploy.sh status
docker stats  # Check memory usage
```

#### Task 5.4: Validation Testing on Pi
**Owner**: tester agent
**Estimated**: 6 hours
**Deliverable**: Validation report

**Actions**:
- Verify MQTT ingestion working (air-quality stream)
- Publish test MQTT message and verify Parquet write
- Query API endpoints (GET /api/v1/readings)
- Add second stream via etcd (weather HTTP poll)
- Verify multi-stream isolation (separate Parquet partitions)
- Run Phase 0 baseline tests on Pi
- Monitor memory and CPU usage for 24 hours

**Acceptance Criteria**:
- All baseline tests pass on Pi
- MQTT ingestion rate matches baseline
- API response times within baseline
- Multi-stream writes to separate partitions
- Memory usage stable over 24 hours
- No OOM kills or crashes

#### Task 5.5: Create Runbook and Documentation
**Owner**: api-docs agent
**Estimated**: 4 hours
**Deliverable**: `/workspaces/neural-data-platform/product/features/air-004/RUNBOOK.md`

**Actions**:
- Document deployment procedure
- Document how to add new streams (etcd workflow)
- Document troubleshooting steps
- Document rollback procedure
- Create Grafana dashboard examples (optional)

**Acceptance Criteria**:
- Runbook covers all operational procedures
- Examples include etcd commands for stream CRUD
- Troubleshooting section addresses common issues
- Rollback procedure tested and validated

### Phase 5 Success Criteria

- [ ] Docker Compose configuration updated
- [ ] Initial stream configs created and loaded
- [ ] Successful deployment to Raspberry Pi 5
- [ ] All services healthy and within memory budget
- [ ] MQTT ingestion working (air-quality stream)
- [ ] Multi-stream capability demonstrated (2+ streams)
- [ ] 24-hour stability test passing
- [ ] Runbook and documentation complete
- [ ] Stakeholder acceptance obtained

**Dependencies**: Phase 4 complete
**Blockers**: Raspberry Pi 5 hardware availability
**Go/No-Go Decision**: Production deployment authorization

---

## Agent Assignments and Coordination

### Primary Agents

**architect agent**
- Phase 0: Document existing interfaces
- Phase 3: Verify MqttSource interface
- Responsible for: Architecture decisions, interface design

**backend-dev agent**
- Phase 1: All type definitions and StreamRegistry
- Phase 2: All storage extensions
- Phase 3: All source implementations
- Phase 4: All coordinator components
- Responsible for: Core Rust implementation

**tester agent**
- Phase 0: Create baseline test suite
- Phase 4: Integration testing
- Phase 5: Validation testing on Pi
- Responsible for: TDD tests, integration tests, regression tests

**perf-analyzer agent**
- Phase 0: Establish performance baseline
- Phase 5: Performance validation on Pi
- Responsible for: Benchmarking, performance monitoring

**cicd-engineer agent**
- Phase 5: Docker configuration, deployment
- Responsible for: Pi deployment, build process

**api-docs agent**
- Phase 5: Runbook and documentation
- Responsible for: Operational documentation

### Coordination Protocol

**Daily Standup (Via Memory Store)**:
- Each agent logs progress to `/workspaces/neural-data-platform/.swarm/memory.db`
- Use `npx claude-flow@alpha hooks post-edit` after each significant change
- Blockers flagged immediately via notifications

**Phase Gates**:
- Phase completion requires sign-off from PLANNER agent
- Go/No-Go decision based on success criteria checklist
- Blockers must be resolved before next phase

**Communication**:
- Use `npx claude-flow@alpha hooks notify` for cross-agent updates
- Critical issues escalated via AIR-004 issue tracker
- Weekly progress reports to stakeholders

---

## Risk Management

### High-Risk Items

**Risk 1: ParquetStore Corruption**
- **Probability**: Medium
- **Impact**: Critical (data loss)
- **Mitigation**:
  - Extensive testing with existing Parquet files (Phase 0, 2)
  - WAL replay testing
  - Backup existing data before Phase 2 deployment
- **Rollback**: Restore from backup, revert to AIR-002

**Risk 2: Memory Limit Exceeded on Pi**
- **Probability**: Medium
- **Impact**: High (OOM kills, service instability)
- **Mitigation**:
  - Conservative memory limits in Docker Compose
  - Monitor memory during 24-hour test (Phase 5)
  - Optimize batch sizes if needed
- **Rollback**: Reduce batch sizes, disable multi-stream mode

**Risk 3: etcd Watch Latency**
- **Probability**: Low
- **Impact**: Medium (delayed stream updates)
- **Mitigation**:
  - Test watch latency in Phase 1
  - Implement debouncing for rapid changes
  - Fallback to polling if watch fails
- **Rollback**: Restart air-quality-app to reload config

**Risk 4: Dual-Write Inconsistency**
- **Probability**: Medium
- **Impact**: Medium (Silver layer incomplete)
- **Mitigation**:
  - Bronze-first write pattern (Phase 2)
  - Retry queue for Silver failures
  - Reconciliation job to detect drift
- **Rollback**: Rebuild Silver from Bronze (backfill)

**Risk 5: ARM64 Build Failures**
- **Probability**: Low
- **Impact**: High (cannot deploy to Pi)
- **Mitigation**:
  - Test ARM64 cross-compilation early (Phase 1)
  - Use multi-stage Docker builds with caching
  - Allow 30-minute build time on Pi
- **Rollback**: Build on x86_64 and use cross-compilation

### Medium-Risk Items

**Risk 6: Source Health Check Failures**
- **Mitigation**: Implement retry with exponential backoff, circuit breaker pattern

**Risk 7: Schema Validation Performance**
- **Mitigation**: Optimize validation logic, cache compiled validators

**Risk 8: TimescaleDB Unavailable**
- **Mitigation**: Silver layer optional, Bronze layer continues working

---

## Success Metrics

### Phase Completion Metrics

| Phase | LOC Added | Tests Added | Integration Tests | Baseline Pass Rate |
|-------|-----------|-------------|-------------------|-------------------|
| Phase 0 | 0 | 10+ | 1 | 100% |
| Phase 1 | 500 | 20+ | 2 | 100% |
| Phase 2 | 800 | 30+ | 4 | 100% |
| Phase 3 | 600 | 25+ | 3 | 100% |
| Phase 4 | 600 | 20+ | 5 | 100% |
| Phase 5 | 0 | 5+ | 1 (Pi) | 100% |

### Performance Targets

**No Regression Allowed** (compared to Phase 0 baseline):

- Config read latency: <10ms (current: <10ms)
- MQTT ingestion rate: >1 msg/sec sustained (current: proven)
- Storage write throughput: >1k records/sec (current: 10k+)
- API response time (p95): <200ms
- Memory usage: <896MB total (Pi constraint)

### Functional Targets

- [ ] Support 3+ heterogeneous streams simultaneously
- [ ] Store 1 million records without data loss
- [ ] Query 30-day window in <5 seconds
- [ ] Add new stream via config (zero code changes)
- [ ] Run for 7 days without manual intervention
- [ ] Raspberry Pi 5 deployment successful

---

## Rollback Procedures

### Phase 1-4 Rollback (Pre-Deployment)

**Trigger**: Any phase gate failure, baseline test regression

**Procedure**:
1. Revert code changes via git
2. Re-run Phase 0 baseline tests
3. Verify air-quality-app still working
4. Analyze failure, update plan, retry phase

**Rollback Time**: <1 hour
**Data Loss**: None (no production deployment yet)

### Phase 5 Rollback (Post-Deployment)

**Trigger**: Production issue on Pi, memory limit exceeded, data corruption

**Procedure**:
```bash
# On Raspberry Pi 5

# 1. Stop all services
cd /workspaces/neural-data-platform/deploy/pi
./deploy.sh stop

# 2. Restore previous Docker images
docker tag air-quality-app:backup air-quality-app:latest

# 3. Restore etcd data from backup
docker run --rm -v pi_etcd-data:/data -v $(pwd)/backups:/backup \
  alpine sh -c "rm -rf /data/* && tar -xzf /backup/etcd-backup.tar.gz -C /data"

# 4. Restart services
./deploy.sh start

# 5. Verify baseline functionality
./deploy.sh status
curl http://localhost:8080/health
```

**Rollback Time**: <15 minutes
**Data Loss**: In-flight data since last backup (max 1 hour)

---

## Timeline Summary

| Phase | Duration | Start Day | End Day | Dependencies |
|-------|----------|-----------|---------|--------------|
| Phase 0: Verification | 1 day | Day 1 | Day 1 | None |
| Phase 1: Foundation | 2-3 days | Day 2 | Day 4 | Phase 0 |
| Phase 2: Storage | 3-4 days | Day 5 | Day 8 | Phase 1 |
| Phase 3: Sources | 3-4 days | Day 9 | Day 12 | Phase 1 |
| Phase 4: Coordination | 4-5 days | Day 13 | Day 17 | Phase 2, 3 |
| Phase 5: Deployment | 2-3 days | Day 18 | Day 20 | Phase 4 |

**Total**: 15-20 days

**Critical Path**: Phase 0 -> Phase 1 -> Phase 2 -> Phase 4 -> Phase 5

**Parallel Work**: Phase 3 (Sources) can overlap with Phase 2 (Storage) after Phase 1

---

## Next Steps

### Immediate Actions (Day 1)

1. **PLANNER agent** (YOU):
   - Save this implementation plan
   - Store plan in memory: `npx claude-flow@alpha hooks post-edit --file "IMPLEMENTATION_PLAN.md" --memory-key "swarm/planner/air004-plan"`
   - Notify team: `npx claude-flow@alpha hooks notify --message "AIR-004 implementation plan complete"`

2. **Coordinate Phase 0 Kickoff**:
   - Assign Task 0.1 to `architect` agent
   - Assign Task 0.2 to `tester` agent
   - Assign Task 0.3 to `perf-analyzer` agent
   - Set deadline: End of Day 1

3. **Stakeholder Review**:
   - Share plan with product owner
   - Confirm Raspberry Pi 5 hardware availability
   - Confirm acceptance criteria alignment

### Phase 0 Kickoff (Today)

```bash
# Initialize pre-task hook
npx claude-flow@alpha hooks pre-task --description "AIR-004 Phase 0: Verification and Protection"

# Architect agent: Document interfaces
# Tester agent: Create baseline tests
# Perf-analyzer agent: Establish metrics

# End of day: Phase 0 review meeting
npx claude-flow@alpha hooks session-end --export-metrics true
```

---

## Appendix A: File Structure

```
/workspaces/neural-data-platform/
├── apps/air-quality-app/src/
│   ├── coordinator/           # NEW (Phase 4)
│   │   ├── ingestion_coordinator.rs
│   │   ├── source_manager.rs
│   │   └── router.rs
│   ├── sources/               # NEW (Phase 3)
│   │   ├── mqtt_wrapper.rs
│   │   ├── http_poller.rs
│   │   └── webhook_handler.rs
│   ├── storage/               # NEW (Phase 2)
│   │   ├── layer_manager.rs
│   │   └── multi_stream_store.rs (optional wrapper)
│   ├── ingestion/             # EXISTING
│   │   └── mqtt_handler.rs    # NO CHANGES
│   ├── pipeline/              # EXISTING
│   │   └── storage_writer.rs  # MINIMAL CHANGES
│   └── config_etcd.rs         # MINIMAL CHANGES (add stream loading)
├── config-client/src/
│   └── stream_registry.rs     # NEW (Phase 1)
├── core/src/
│   ├── types/                 # NEW (Phase 1)
│   │   ├── stream_record.rs
│   │   └── stream_config.rs
│   └── storage/
│       ├── parquet.rs         # EXTEND (Phase 2)
│       └── timescale.rs       # NEW (Phase 2)
├── deploy/pi/
│   ├── docker-compose.yml     # EXTEND (Phase 5)
│   └── configs/streams/       # NEW (Phase 5)
└── product/features/air-004/
    ├── SPECIFICATION.md       # EXISTING
    ├── DEPENDENCY_MAP.md      # EXISTING
    ├── PSEUDOCODE.md          # EXISTING
    ├── IMPLEMENTATION_PLAN.md # THIS FILE
    ├── BASELINE_METRICS.md    # NEW (Phase 0)
    ├── RUNBOOK.md             # NEW (Phase 5)
    └── architecture/
        └── EXISTING_INTERFACES.md  # NEW (Phase 0)
```

---

## Appendix B: Testing Strategy

### London TDD Approach

**Every component follows this pattern**:

1. **Define Interface** (mock boundary)
2. **Write Mock-Based Unit Test** (test behavior, not implementation)
3. **Implement Component** (satisfy mocks)
4. **Integration Test** (real dependencies)

**Example: HttpPoller**

```rust
// Step 1: Define trait
#[async_trait]
pub trait HttpPollerTrait {
    async fn poll_once(&self) -> Result<Vec<TimeSeriesPoint>, Error>;
}

// Step 2: Mock-based unit test
#[cfg(test)]
mod tests {
    use mockall::mock;

    mock! {
        HttpClient {}
        impl HttpPollerTrait for HttpClient {
            async fn poll_once(&self) -> Result<Vec<TimeSeriesPoint>, Error>;
        }
    }

    #[tokio::test]
    async fn test_http_poller_success() {
        let mut mock_client = MockHttpClient::new();
        mock_client.expect_poll_once()
            .returning(|| Ok(vec![/* test points */]));

        let poller = HttpPoller::new_with_client(mock_client);
        let result = poller.poll_once().await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
}

// Step 3: Implement
impl HttpPoller {
    async fn poll_once(&self) -> Result<Vec<TimeSeriesPoint>, Error> {
        // Real implementation
    }
}

// Step 4: Integration test
#[tokio::test]
async fn integration_test_http_poller_real_endpoint() {
    let config = HttpPollSourceConfig { /* ... */ };
    let poller = HttpPoller::new(config);
    let result = poller.poll_once().await;
    // Test with real HTTP server (wiremock)
}
```

---

## Appendix C: Memory Budget Breakdown (Pi)

**Total Available**: 16GB RAM
**Platform Services Budget**: 896MB

| Service | Allocated | Actual (Baseline) | Headroom |
|---------|-----------|-------------------|----------|
| mosquitto | 50MB | ~40MB | 10MB |
| etcd | 300MB | ~250MB | 50MB |
| air-quality-app | 500MB | ~350MB | 150MB |
| TimescaleDB | (not on Pi) | N/A | N/A |
| **Total** | **850MB** | **~640MB** | **210MB** |

**Multi-Stream Overhead Estimate**: +50-100MB
**Post-AIR-004 Total**: ~740MB
**Safety Margin**: 156MB (18%)

---

*End of Implementation Plan*

**Document Status**: Complete and Ready for Execution
**Next Review**: After Phase 0 completion
**Maintained By**: PLANNER agent
**Last Updated**: 2025-12-15
