# Reference Architectures and Prior Art for Time-Series Intelligence Platforms

**Research Date:** 2025-12-13
**Purpose:** Identify architectural patterns, best practices, and lessons learned from existing time-series intelligence platforms to inform the design of a domain-agnostic platform.

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Open-Source Air Quality Monitoring Platforms](#open-source-air-quality-monitoring-platforms)
3. [AirGradient Software Stack](#airgradient-software-stack)
4. [Generic Time-Series Intelligence Platforms](#generic-time-series-intelligence-platforms)
5. [Event-Driven Architectures in Rust](#event-driven-architectures-in-rust)
6. [Hexagonal/Ports-and-Adapters Patterns](#hexagonalports-and-adapters-patterns)
7. [Time-Series Database Architectures](#time-series-database-architectures)
8. [Plugin and Extensibility Patterns](#plugin-and-extensibility-patterns)
9. [CQRS and Event Sourcing](#cqrs-and-event-sourcing)
10. [Observability and Monitoring Architecture](#observability-and-monitoring-architecture)
11. [Key Architectural Patterns Summary](#key-architectural-patterns-summary)
12. [Lessons Learned](#lessons-learned)
13. [Best Practices for Extensibility](#best-practices-for-extensibility)
14. [Trade-offs Analysis](#trade-offs-analysis)
15. [Recommendations](#recommendations)

---

## Executive Summary

This research examines reference architectures from multiple domains to inform the design of a domain-agnostic time-series intelligence platform. Key findings include:

- **Domain-Agnostic Design**: Modern platforms are moving toward domain-agnostic architectures that can handle multiple use cases (AIOps, IoT, financial data, etc.)
- **Hexagonal Architecture**: Ports-and-adapters pattern provides excellent domain isolation and testability
- **Event-Driven Patterns**: Actor models and message-passing architectures (via tokio/actix) offer robust concurrency
- **Time-Series Foundation Models**: Emerging trend of AI-powered time-series analysis platforms (2024-2025)
- **Extensibility via Plugins**: WebAssembly and dynamic loading enable runtime extensibility
- **Observability-First**: OpenTelemetry, Prometheus, and Grafana represent best practices for monitoring

---

## 1. Open-Source Air Quality Monitoring Platforms

### 1.1 Major Platforms

#### **AirGradient**
- **Architecture**: Fully open-source hardware and software under CC license
- **Data Flow**: Sensors → Local/Cloud Platform → Dashboard/API
- **Flexibility**: Data can be sent to any server (Home Assistant, Homey, custom platforms)
- **Community**: Thousands of monitors across 80+ countries
- **Key Pattern**: Separation of hardware, data collection, and visualization layers

**Architectural Insights:**
- Modular design allows users to choose their own data platform
- Strong community-driven development model
- Integration-first approach (Home Assistant, HomeKit, MQTT)

#### **OpenAQ**
- **Architecture**: First centralized open-source air quality data aggregation platform
- **Scale**: Largest open-source air quality platform globally
- **Data Sources**: Government reference monitors (2015+) and sensors (2021+)
- **Key Pattern**: Centralized aggregation with FAIR principles (Findable, Accessible, Interoperable, Reusable)

**Architectural Insights:**
- ETL pipeline for heterogeneous data sources
- Emphasis on data standardization and interoperability
- API-first design for data access

#### **EnviroMonitor**
- **Architecture**: Community-driven global monitoring network
- **Measurements**: PM2.5, PM10, temperature, humidity, barometric pressure
- **Key Pattern**: Distributed data collection with centralized aggregation

#### **AQuality32**
- **Hardware**: ESP32-based with multiple sensors (CO2, PM, temp/humidity)
- **Power**: Battery-powered for mobile/stationary use
- **Target**: Small research teams requiring affordable solutions
- **Key Pattern**: Low-cost, open-source hardware for research applications

### 1.2 Common Architectural Patterns

1. **Three-Tier Architecture**:
   - Hardware Layer: Sensors and data collection devices
   - Application Layer: Data processing and storage
   - Presentation Layer: Visualization and APIs

2. **Data Democratization**:
   - Open APIs for data access
   - Citizen science participation
   - Global data repositories

3. **Challenges Identified**:
   - Funding sustainability (2025 saw cutbacks to EPA EJScreen, NASA SERVIR)
   - Data continuity and long-term storage
   - Calibration and data quality assurance

---

## 2. AirGradient Software Stack

### 2.1 API Architecture

**Dual API Design:**
- **Public API**: Cloud-hosted data via AirGradient dashboard
- **Local API**: Direct device access over local network

**Security Model:**
- Token-based authentication for public API
- Network-isolated local API (same WiFi only)

### 2.2 Data Format

**JSON Response Structure:**
```json
{
  "wifi": "<signal_strength>",
  "boot": "<boot_count>",
  "serialno": "<device_id>",
  "rco2": "<co2_ppm>",
  "pm01": "<pm1.0_ug/m3>",
  "pm02": "<pm2.5_ug/m3>",
  "pm10": "<pm10_ug/m3>",
  "pm003_count": "<particle_count>",
  "atmp": "<temperature_c>",
  "rhum": "<humidity_%>",
  "tvoc_raw": "<tvoc_raw_value>"
}
```

### 2.3 Technology Stack

**Hardware:**
- ESP32-C3-MINI (current generation)
- Wemos D1 Mini (legacy)

**Backend:**
- PostgreSQL with PostGIS extension (geospatial)
- pg_timeseries extension (time-series optimization)
- Cron-based data retrieval tasks

**Protocols:**
- HTTP/REST for API access
- MQTT with basic authentication
- Swagger/OpenAPI documentation

### 2.4 Integration Patterns

1. **Platform Integrations**: Home Assistant, Homey, openHAB
2. **Protocol Support**: HTTP, MQTT, custom APIs
3. **Data Export**: Real-time streaming and historical data access

### 2.5 Architectural Lessons

**Strengths:**
- Clear separation between local and cloud access
- Flexible integration options
- Simple, well-documented JSON API

**Considerations:**
- Token management for API access
- Network isolation for local API limits remote access
- Need for both real-time and historical data access patterns

---

## 3. Generic Time-Series Intelligence Platforms

### 3.1 Emerging Trends (2024-2025)

#### **Cisco Data Fabric (September 2025)**
- **Platform**: Splunk-powered AI-ready data fabric
- **Key Innovation**: Time Series Foundation Model
- **Capabilities**:
  - Advanced pattern analysis and temporal reasoning
  - Anomaly detection and forecasting
  - Automated root cause analysis
  - Proactive operations and incident response

**Architectural Pattern:**
- Unified data fabric for machine and business data
- Foundation models for time-series intelligence
- Agent-based workflows for automation

#### **Domain-Agnostic AIOps Platforms**
- **Trend**: Moving from narrow (network/database-specific) to broad (cross-domain) platforms
- **Gartner Direction**: Emphasis on domain-agnostic platforms spanning infrastructure and applications
- **Key Pattern**: Unified observability across multiple data sources

### 3.2 Time-Series Foundation Models

**Major Releases (2024-2025):**
- **TimesFM** (Google)
- **Chronos** (Amazon)
- **Moirai** (Salesforce)
- **TimeGPT**
- **Lag-LLama** (ServiceNow)
- **Timer-XL** (THUML)

**Key Characteristics:**
- Pre-trained on diverse time-series datasets
- Transfer learning capabilities
- Domain-agnostic forecasting
- Integration with generative AI

### 3.3 Deep Learning Architectures for Time Series

**Fundamental Architectures:**
- **MLPs**: Multi-layer perceptrons for simple patterns
- **CNNs**: Convolutional networks for local patterns
- **RNNs/LSTMs**: Recurrent networks for sequential dependencies
- **GNNs**: Graph neural networks for relational time series
- **Transformers**: Attention-based models for long-range dependencies

**Evaluation Challenges:**
- Limited benchmark dataset diversity
- Comparable performance across architectures
- Need for domain-specific evaluation

### 3.4 C3 AI's Approach

**Key Innovation:**
- Deep integration between time-series systems and generative AI
- Agent interface for time-series data interaction
- Automated prediction and decision intelligence

**Architectural Pattern:**
- Time-series data as first-class citizens in AI workflows
- Natural language interface to time-series operations
- Automated forecasting and anomaly detection

### 3.5 Modern Data Platform Architecture (2025)

**Gartner Principles:**
- **Modular**: Component-based architecture
- **Scalable**: Horizontal and vertical scaling
- **Adaptable**: Flexible to changing requirements

**Key Components:**
1. Data ingestion layer
2. Storage and processing layer
3. Analytics and ML layer
4. Consumption and visualization layer
5. Governance and security layer

---

## 4. Event-Driven Architectures in Rust

### 4.1 Actor Model Fundamentals

**Core Concepts:**
- Actors encapsulate state and behavior
- Communication exclusively through messages
- Single-threaded message processing (no race conditions)
- Location transparency

### 4.2 Actix Framework

**Architecture:**
- Built on top of Tokio runtime
- Multiple actors per thread
- Multi-threaded via Arbiter API
- Typed message passing

**Key Features:**
- Type-safe actor communication
- Automatic message routing
- Lifecycle management
- Supervision strategies

**Use Cases:**
- Web servers
- Game engines
- Microservices
- Concurrent data processing

**Example Pattern:**
```rust
// Actor definition
struct DataProcessor {
    state: ProcessorState,
}

impl Actor for DataProcessor {
    type Context = Context<Self>;
}

// Message handler
impl Handler<ProcessMessage> for DataProcessor {
    type Result = ProcessResult;

    fn handle(&mut self, msg: ProcessMessage, ctx: &mut Context<Self>) -> Self::Result {
        // Process message atomically
    }
}
```

### 4.3 Building Actors with Tokio

**Direct Tokio Approach:**
- Use `tokio::spawn` for actor tasks
- Message passing via `tokio::sync::mpsc` channels
- Custom handler implementations
- No external actor library required

**Benefits:**
- Full control over actor behavior
- Minimal dependencies
- Integration with existing Tokio code

**Considerations:**
- Manual lifecycle management
- Custom supervision logic
- Careful handling of actor cycles

**Example Pattern:**
```rust
struct ActorHandle {
    sender: mpsc::Sender<Message>,
}

async fn actor_task(mut receiver: mpsc::Receiver<Message>) {
    while let Some(msg) = receiver.recv().await {
        // Handle message
    }
}

// Spawn actor
let (tx, rx) = mpsc::channel(100);
tokio::spawn(actor_task(rx));
let handle = ActorHandle { sender: tx };
```

### 4.4 Message Passing Libraries

#### **Tokio Channels**
- **Types**: mpsc (multi-producer, single-consumer), oneshot, broadcast, watch
- **Features**: Async/await native, bounded/unbounded
- **Integration**: Seamless with Tokio ecosystem

#### **Crossbeam Channels**
- **Type**: Multi-producer, multi-consumer (MPMC)
- **Features**: Mature, highly optimized
- **Use Case**: Sync/blocking scenarios
- **Integration**: Companion crates for async support

#### **Flume**
- **Type**: MPMC with zero unsafe code
- **Performance**: Often faster than std::sync::mpsc and competitive with crossbeam
- **Features**: Selector API, async support, bounded/unbounded/rendezvous
- **Status**: Casual maintenance mode (stable, security/bug fixes only)

**Performance Comparison:**
- Flume: Low latency, small memory footprint
- Crossbeam: Battle-tested, comprehensive ecosystem
- Tokio: Best for async-first applications

### 4.5 Event-Driven Architecture Patterns

#### **Event Loop Pattern**
```rust
async fn event_loop(mut receiver: mpsc::Receiver<Event>) {
    while let Some(event) = receiver.recv().await {
        match event {
            Event::DataArrived(data) => handle_data(data),
            Event::Shutdown => break,
        }
    }
}
```

#### **Backpressure Handling**
- Bounded channels automatically apply backpressure
- Sender blocks when channel is full
- Prevents memory overflow in high-throughput scenarios

#### **Message Bus Architecture**
- **SEDA (Staged Event-Driven Architecture) Bus**: Form of message bus avoiding thread overhead
- **Pattern**: Each component has inbound/outbound queues
- **Benefit**: Isolated concurrent components with sequential event loops

### 4.6 Recommendations for Time-Series Platform

1. **For Simple Message Passing**: Use Tokio's native channels
2. **For Complex Actor Systems**: Consider Actix framework
3. **For High-Performance MPMC**: Use Flume or Crossbeam
4. **For Event-Driven Pipelines**: Implement SEDA-style architecture

**Architectural Pattern:**
```
Data Source → Ingestion Actor → Processing Actors → Storage Actor → Query Actor
                     ↓                    ↓
              [backpressure]      [parallel processing]
```

---

## 5. Hexagonal/Ports-and-Adapters Patterns

### 5.1 Conceptual Foundation

**Origin**: Alistair Cockburn
**Goal**: Decouple domain logic from infrastructure concerns
**Visual**: Hexagon shape represents adapters surrounding the business domain core

### 5.2 Core Concepts

#### **Ports**
- Interfaces/abstractions defining communication with external systems
- Specified by the domain (hexagon)
- Do not dictate implementation details
- Two types:
  - **Driving Ports** (Inbound): Application use cases, consumed by primary adapters
  - **Driven Ports** (Outbound): Infrastructure requirements, implemented by secondary adapters

#### **Adapters**
- Translate external requests/responses to/from domain format
- **Primary Adapters** (Driving): REST controllers, CLI, web UI, GraphQL
- **Secondary Adapters** (Driven): Databases, message queues, external APIs, file systems

### 5.3 Rust Implementation Patterns

#### **Traits for Ports**
```rust
// Domain port (trait)
pub trait SensorRepository {
    async fn save(&self, sensor: &Sensor) -> Result<(), Error>;
    async fn find_by_id(&self, id: &SensorId) -> Result<Option<Sensor>, Error>;
}

// Domain service
pub struct SensorService {
    repository: Box<dyn SensorRepository>,
}

impl SensorService {
    pub async fn register_sensor(&self, sensor: Sensor) -> Result<(), Error> {
        self.repository.save(&sensor).await
    }
}
```

#### **Adapters Implementing Ports**
```rust
// PostgreSQL adapter
pub struct PostgresSensorRepository {
    pool: PgPool,
}

impl SensorRepository for PostgresSensorRepository {
    async fn save(&self, sensor: &Sensor) -> Result<(), Error> {
        // PostgreSQL-specific implementation
    }
}

// In-memory adapter (for testing)
pub struct InMemorySensorRepository {
    sensors: Arc<RwLock<HashMap<SensorId, Sensor>>>,
}

impl SensorRepository for InMemorySensorRepository {
    async fn save(&self, sensor: &Sensor) -> Result<(), Error> {
        // In-memory implementation
    }
}
```

### 5.4 Project Structure

**Crate Organization:**
```
crates/
├── platform-core/         # Domain and business logic
│   ├── entities/
│   ├── services/
│   └── ports/            # Trait definitions
├── platform-adapters/    # Infrastructure implementations
│   ├── postgres/
│   ├── influxdb/
│   ├── mqtt/
│   └── in_memory/
├── platform-api/         # REST API (primary adapter)
└── platform-cli/         # CLI (primary adapter)
```

**Benefits:**
- Each crate has clear responsibilities
- Domain code has no infrastructure dependencies
- Easy to swap implementations (e.g., PostgreSQL → InfluxDB)
- Excellent testability

### 5.5 Key Implementation Guidelines

1. **Port Input/Output Must Be Domain Types**:
   - Ports return domain entities, not adapter-specific types
   - Prevents domain from depending on infrastructure

2. **Avoid Boxing When Possible**:
   - Use generics instead of `Box<dyn Trait>` where feasible
   - Consider const generics and zero-cost abstractions

3. **Primary Adapters Depend on Domain**:
   - REST controllers import domain services
   - No need for interfaces on use cases (incoming ports)

4. **Secondary Adapters Implement Domain Interfaces**:
   - Domain defines repository traits
   - Adapters provide concrete implementations

### 5.6 Benefits for Time-Series Platforms

**Testability:**
```rust
// Easy unit testing with mock repositories
#[tokio::test]
async fn test_sensor_registration() {
    let repo = InMemorySensorRepository::new();
    let service = SensorService::new(Box::new(repo));

    let sensor = Sensor::new("sensor-1", SensorType::Temperature);
    service.register_sensor(sensor).await.unwrap();
}
```

**Flexibility:**
- Swap time-series databases without changing domain logic
- Support multiple data sources simultaneously
- A/B test different storage backends

**Maintainability:**
- Infrastructure changes isolated to adapter layer
- Domain logic remains stable
- Clear boundaries reduce cognitive load

### 5.7 Considerations

**Trade-offs:**
- More OOP-oriented than idiomatic Rust
- Can introduce complexity for simple applications
- Boxing traits has runtime cost (consider generics)

**When to Use:**
- Long-term maintenance requirements
- Multiple infrastructure implementations
- Complex domain logic requiring isolation
- Testing is a priority

**When NOT to Use:**
- Simple CRUD applications
- No significant business logic
- Single infrastructure target
- Performance-critical paths (consider zero-cost abstractions)

### 5.8 Domain-Agnostic Application

For a time-series intelligence platform:

1. **Core Domain** (platform-core):
   - Time-series data model (domain-agnostic)
   - Analysis algorithms
   - Query abstractions
   - Transformation pipelines

2. **Domain-Specific Adapters** (platform-adapters):
   - Air quality sensor parsing
   - Financial data normalization
   - IoT protocol handlers
   - Custom metric collectors

3. **Storage Adapters**:
   - InfluxDB
   - TimescaleDB
   - QuestDB
   - Custom time-series stores

4. **API Adapters**:
   - REST API
   - GraphQL
   - gRPC
   - WebSocket streams

---

## 6. Time-Series Database Architectures

### 6.1 Comparative Analysis

| Database | Architecture | Storage Model | Best For | Limitations |
|----------|--------------|---------------|----------|-------------|
| **InfluxDB** | Time-Structured Merge (TSM) tree | Log-structured merge tree | Established ecosystem, streaming | High cardinality issues |
| **TimescaleDB** | Hybrid row-columnar (Hypercore) | Hypertables with auto-partitioning | PostgreSQL compatibility, complex queries | Schema configuration required |
| **QuestDB** | Three-tier columnar | WAL + columnar partitions | High-speed writes, high cardinality | Smaller community |

### 6.2 InfluxDB Architecture

**Storage Engine:**
- **TSM (Time-Structured Merge) Tree**: LSM-tree variant optimized for time-series
- **Compression**: Specialized algorithms for time-series data
- **Indexing**: Time-based and tag-based indexing

**Design Decisions:**
- Each time series stored in its own TSM tree
- Optimized for write-heavy workloads
- Downsample and retention policies built-in

**Performance Characteristics:**
- **Strength**: High write throughput for moderate cardinality
- **Weakness**: Performance degrades with high cardinality (many unique series)
- **Reason**: Separate TSM trees per series increase read/write costs

**Use Cases:**
- IoT sensor data collection
- Application metrics
- Real-time analytics

### 6.3 TimescaleDB Architecture

**Foundation:**
- Extends PostgreSQL with time-series optimizations
- Maintains full SQL compatibility

**Storage Innovation:**
- **Hypercore**: Hybrid row-columnar storage engine
  - Recent data: Row format (fast inserts)
  - Old data: Automatically converted to columnar format with compression
- **Hypertables**: Abstraction over chunked data
  - Automatic partitioning by time intervals
  - Optional space partitioning
  - Transparent to queries

**Performance Characteristics:**
- **Strength**: Complex analytical queries, better high-cardinality handling than InfluxDB
- **Weakness**: Ingest rate limited by PostgreSQL architecture
- **Schema**: Requires upfront configuration

**Use Cases:**
- PostgreSQL-native environments
- Complex analytical workloads
- Applications needing ACID guarantees

### 6.4 QuestDB Architecture

**Three-Tier Storage:**

1. **Tier 1 - Write-Ahead Log (WAL)**:
   - Buffers incoming writes for durability
   - Handles out-of-order data
   - Enables high-throughput ingestion (millions of rows/second)

2. **Tier 2 - Columnar Partitions**:
   - Time-partitioned columnar storage
   - Separate files per column
   - Efficient compression and selective reads

3. **Tier 3 - Query Execution**:
   - SIMD-accelerated scans
   - Custom JIT compiler for parallel filters
   - Time-aware query optimization

**Storage Model:**
- **Densely ordered vectors**: Unlike LSM trees or B-trees
- **Column-oriented**: Each column stored separately
- **Time-partitioned**: Automatic partitioning by time ranges

**Performance Characteristics:**
- **Strength**: Fastest open-source time-series database for ingestion, excellent high-cardinality support
- **Query Speed**: SIMD instructions and parallel processing
- **Weakness**: Smaller ecosystem and fewer integrations

**Protocol Support:**
- InfluxDB line protocol (drop-in replacement)
- PostgreSQL wire protocol
- REST API for bulk operations
- Schema-agnostic ingestion

### 6.5 Common Time-Series Workload Patterns

**Characteristics:**
- High-frequency data ingestion
- Time-based indexing
- Multi-dimensional data (tags/labels)
- Correlated data points
- Aggregation and downsampling
- Retention policies

**Write Patterns:**
- Append-only writes
- Out-of-order handling
- Batch vs. streaming ingestion

**Read Patterns:**
- Range queries (time-based)
- Aggregations (sum, avg, percentile)
- Downsampling for visualization
- Real-time queries vs. historical analysis

### 6.6 Architectural Lessons for Time-Series Platform

#### **Storage Strategy Selection Matrix**

| Requirement | Recommended Approach |
|-------------|---------------------|
| PostgreSQL compatibility | TimescaleDB |
| Maximum write throughput | QuestDB |
| Established ecosystem | InfluxDB |
| High cardinality data | QuestDB or TimescaleDB |
| Complex analytical queries | TimescaleDB |
| Schema flexibility | QuestDB (schema-agnostic) |

#### **Key Design Patterns**

1. **Columnar Storage for Analytics**:
   - Separate column files enable efficient compression
   - Selective column reads reduce I/O
   - SIMD-friendly data layout

2. **Time Partitioning**:
   - Automatic partitioning by time ranges
   - Efficient time-range queries
   - Easy data retention management

3. **Write-Ahead Log**:
   - Durability without sacrificing write speed
   - Buffer for out-of-order data
   - Crash recovery

4. **Hybrid Storage**:
   - Row-based for recent (hot) data
   - Columnar for historical (cold) data
   - Automatic tiering based on age

5. **Schema Flexibility vs. Performance**:
   - Schema-agnostic (QuestDB): Easier ingestion, flexible
   - Schema-enforced (TimescaleDB): Better query optimization, type safety

#### **Recommendations for Domain-Agnostic Platform**

1. **Abstract Storage Layer**:
   - Define storage traits (ports)
   - Implement adapters for multiple backends
   - Allow runtime backend selection

2. **Leverage Protocol Standards**:
   - Support InfluxDB line protocol (widely adopted)
   - PostgreSQL wire protocol for SQL compatibility
   - OpenTelemetry for observability integration

3. **Optimize for Common Patterns**:
   - Columnar storage for analytical queries
   - Time partitioning for efficient range queries
   - WAL for high-throughput writes

4. **Plan for High Cardinality**:
   - Avoid per-series indexes if possible
   - Use inverted indexes for tags
   - Consider QuestDB-style dense vector storage

---

## 7. Plugin and Extensibility Patterns

### 7.1 Overview of Extensibility Requirements

For a domain-agnostic time-series platform, extensibility is critical:
- Support custom data sources
- Allow domain-specific transformations
- Enable custom analysis algorithms
- Integrate with various output systems

### 7.2 Dynamic Loading in Rust

**Mechanism:**
- OS-provided dynamic loading (`dlopen()` on *nix, LoadLibrary on Windows)
- `libloading` crate provides safe Rust interface
- Load libraries at runtime and resolve symbols

**Challenges:**
- **ABI Stability**: Rust does not guarantee stable ABI across compiler versions
- **Safety**: Requires `unsafe` code for FFI
- **Cross-Platform**: Different extensions (.dll, .so, .dylib)

### 7.3 Plugin Architecture Patterns

#### **1. Trait-Based Plugin Interface**

```rust
// Define plugin trait in core crate
pub trait DataSourcePlugin {
    fn name(&self) -> &str;
    fn ingest(&self, config: &PluginConfig) -> Result<Vec<DataPoint>, Error>;
}

// Plugin implementation (in separate crate)
pub struct MqttDataSource;

impl DataSourcePlugin for MqttDataSource {
    fn name(&self) -> &str { "mqtt" }
    fn ingest(&self, config: &PluginConfig) -> Result<Vec<DataPoint>, Error> {
        // MQTT-specific ingestion logic
    }
}

// Dynamic loading (requires careful ABI handling)
type PluginCreate = fn() -> Box<dyn DataSourcePlugin>;

let lib = Library::new("plugins/mqtt.so")?;
let constructor: Symbol<PluginCreate> = lib.get(b"create_plugin")?;
let plugin = constructor();
```

#### **2. WebAssembly Plugins**

**Advantages:**
- No `unsafe` code required
- Sandboxed execution (security)
- Cross-platform (WASM runs anywhere)
- Predictable performance

**Trade-offs:**
- Slight performance overhead vs. native
- Limited host interaction (need WASI or custom imports)

**Example Frameworks:**
- **Wasmtime**: Fast and secure WASM runtime
- **Wasmer**: Universal WASM runtime with WASI support
- **wasm3**: Lightweight interpreter

**Use Case for Time-Series Platform:**
```rust
// Host code
let engine = wasmtime::Engine::default();
let module = wasmtime::Module::from_file(&engine, "plugins/custom_analyzer.wasm")?;
let mut store = wasmtime::Store::new(&engine, ());
let instance = wasmtime::Instance::new(&mut store, &module, &[])?;

// Call plugin function
let analyze = instance.get_typed_func::<(f64,), f64>(&mut store, "analyze")?;
let result = analyze.call(&mut store, (data_point,))?;
```

#### **3. CLI-Based Extensions**

**Pattern:**
- Plugins are separate binaries with naming convention (e.g., `platform-plugin-mqtt`)
- Host discovers plugins via $PATH
- Communication via stdin/stdout (JSON)

**Examples:**
- Cargo's plugin system (`cargo-expand`, etc.)
- mdbook plugins

**Benefits:**
- Simplest implementation
- Language-agnostic plugins
- No ABI concerns

**Example:**
```rust
// Host invokes plugin
let output = Command::new("platform-plugin-custom")
    .arg("--config")
    .arg(config_path)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;

// Send data via stdin, read results from stdout
```

### 7.4 Real-World Examples

#### **Bevy Game Engine**
- Feature plugins (usually compile-time)
- `bevy::dynamic_plugin` for runtime loading
- Uses `libloading` internally
- Minimal code overhead

#### **Zellij Terminal Workspace**
- Plugin system using WebAssembly
- Plugins written in any language compiling to WASM
- Sandboxed execution for security

### 7.5 Performance Considerations

**Dynamic Loading vs. Static Linking:**
- **Dynamic**: 38% faster compile times (plugins compile independently)
- **Static**: Better runtime performance (no dynamic dispatch overhead)
- **Recommendation**: Dynamic for development, static for production builds

### 7.6 Cross-Platform Plugin Loading

**File Extensions:**
- Windows: `.dll`
- Linux/FreeBSD: `.so`
- macOS: `.dylib`

**Recommendation:**
- Use single extension (e.g., `.module`) for all platforms
- `libloading` supports custom extensions

### 7.7 Plugin Discovery Mechanisms

1. **Directory Scanning**:
   ```rust
   let plugin_dir = PathBuf::from("plugins/");
   for entry in fs::read_dir(plugin_dir)? {
       let path = entry?.path();
       if path.extension() == Some("module") {
           load_plugin(&path)?;
       }
   }
   ```

2. **Configuration File**:
   ```toml
   [plugins]
   mqtt = { path = "plugins/mqtt.module" }
   http = { path = "plugins/http.module" }
   ```

3. **Environment Variable**:
   ```bash
   PLATFORM_PLUGINS=/path/to/plugins1:/path/to/plugins2
   ```

### 7.8 Recommendations for Time-Series Platform

**Plugin Types Needed:**

1. **Data Source Plugins**:
   - MQTT, HTTP, gRPC, Kafka, custom protocols
   - Each plugin implements `DataSourcePlugin` trait

2. **Transformation Plugins**:
   - Domain-specific data normalization
   - Unit conversions, calibration
   - Implements `TransformPlugin` trait

3. **Analysis Plugins**:
   - Custom algorithms, ML models
   - Anomaly detection, forecasting
   - Implements `AnalysisPlugin` trait

4. **Output Plugins**:
   - Custom visualizations, alerts, integrations
   - Implements `OutputPlugin` trait

**Recommended Approach:**

- **Development**: WebAssembly plugins for maximum safety and flexibility
- **Production**: Option for statically-linked plugins (compile-time) for performance
- **Discovery**: Configuration file with explicit plugin paths
- **Versioning**: Semantic versioning for plugin API compatibility

**Plugin API Stability:**
```rust
// Version plugin API
pub const PLUGIN_API_VERSION: u32 = 1;

pub trait Plugin {
    fn api_version(&self) -> u32 { PLUGIN_API_VERSION }
    // ... other methods
}
```

---

## 8. CQRS and Event Sourcing

### 8.1 Overview

**CQRS (Command Query Responsibility Segregation)**:
- Separate write models (commands) from read models (queries)
- Optimizes each for its specific purpose
- Common in Domain-Driven Design (DDD)

**Event Sourcing**:
- Store state changes as immutable events
- Events are the single source of truth
- Current state derived by replaying events

### 8.2 Core Concepts

#### **Aggregate**
- Fundamental component encapsulating state and business logic
- Composed of DDD entities and value objects
- Ensures business invariants
- Receives commands, emits events

#### **Domain Events**
- Immutable records of business state changes
- Single source of truth in event sourcing
- Enable audit trails and time travel

#### **Command**
- Request to change state
- Validated by aggregate
- Either succeeds (emits events) or fails

#### **Query**
- Read-only operation
- Accesses optimized read models
- No side effects

### 8.3 Rust Implementation

#### **cqrs-es Framework**

```rust
use cqrs_es::{Aggregate, DomainEvent, Command};

// Define aggregate
pub struct SensorAggregate {
    sensor_id: String,
    status: SensorStatus,
    readings: Vec<Reading>,
}

// Define events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorEvent {
    SensorRegistered { sensor_id: String, sensor_type: String },
    ReadingRecorded { timestamp: DateTime<Utc>, value: f64 },
    SensorDeactivated { reason: String },
}

impl DomainEvent for SensorEvent {
    fn event_type(&self) -> String {
        match self {
            SensorEvent::SensorRegistered { .. } => "SensorRegistered".to_string(),
            SensorEvent::ReadingRecorded { .. } => "ReadingRecorded".to_string(),
            SensorEvent::SensorDeactivated { .. } => "SensorDeactivated".to_string(),
        }
    }

    fn event_version(&self) -> String {
        "1.0".to_string()
    }
}

// Implement aggregate
impl Aggregate for SensorAggregate {
    type Command = SensorCommand;
    type Event = SensorEvent;
    type Error = SensorError;
    type Services = SensorServices;

    fn aggregate_type() -> String {
        "Sensor".to_string()
    }

    fn handle(&self, command: Self::Command, _services: &Self::Services)
        -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            SensorCommand::RegisterSensor { sensor_id, sensor_type } => {
                // Validate command
                if self.sensor_id.is_empty() {
                    Ok(vec![SensorEvent::SensorRegistered { sensor_id, sensor_type }])
                } else {
                    Err(SensorError::AlreadyRegistered)
                }
            }
            SensorCommand::RecordReading { timestamp, value } => {
                Ok(vec![SensorEvent::ReadingRecorded { timestamp, value }])
            }
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            SensorEvent::SensorRegistered { sensor_id, .. } => {
                self.sensor_id = sensor_id;
                self.status = SensorStatus::Active;
            }
            SensorEvent::ReadingRecorded { timestamp, value } => {
                self.readings.push(Reading { timestamp, value });
            }
            SensorEvent::SensorDeactivated { .. } => {
                self.status = SensorStatus::Inactive;
            }
        }
    }
}
```

#### **event_sourcing.rs (Prima.it)**

**Features:**
- PostgreSQL backend via sqlx
- UUID, JSON, chrono support
- Opinionated CQRS/ES implementation

**Architecture:**
```rust
// Define aggregate
#[derive(Default)]
struct SensorAggregate {
    // state fields
}

// Define events
#[derive(Serialize, Deserialize)]
enum SensorEvent {
    Registered { id: Uuid },
    Updated { value: f64 },
}

// Implement event handler
impl EventHandler for SensorAggregate {
    type Event = SensorEvent;

    fn handle_event(&mut self, event: Self::Event) {
        match event {
            SensorEvent::Registered { id } => {
                // Update state
            }
            SensorEvent::Updated { value } => {
                // Update state
            }
        }
    }
}
```

#### **fmodel-rust**

**Key Features:**
- Tactical DDD patterns optimized for ES/CQRS
- Runs in single-threaded, multi-threaded, or distributed environments
- Functional modeling approach

### 8.4 Benefits for Time-Series Platforms

**1. Complete Audit Trail**:
- Every sensor reading stored as immutable event
- Historical analysis and compliance
- Debugging and troubleshooting

**2. Time Travel**:
- Reconstruct system state at any point in time
- Replay events for analysis
- Test new algorithms on historical data

**3. Scalable Reads**:
- Separate read models optimized for queries
- Denormalized views for fast access
- Multiple projections for different use cases

**4. Event-Driven Architecture**:
- Natural fit with streaming data
- Real-time processing and alerting
- Integration with event buses (Kafka, NATS)

**5. Testability**:
- Unit test business logic in aggregates
- Test event handlers independently
- No database required for domain tests

### 8.5 Architectural Patterns

#### **Write Side (Command)**
```
Command → Aggregate → Validate → Emit Events → Event Store
                                                    ↓
                                              Event Publisher
```

#### **Read Side (Query)**
```
Event Store → Event Handlers → Update Projections → Query Models
                                                          ↓
                                                    Query API
```

#### **Time-Series Specific Pattern**
```
Sensor Reading (Command) → Sensor Aggregate → ReadingRecorded (Event)
                                                    ↓
                                        ┌───────────┴───────────┐
                                        ↓                       ↓
                                Time-Series DB          Real-Time Projection
                               (historical data)         (current values)
```

### 8.6 Trade-offs

**Pros:**
- Immutable event log (audit, compliance)
- Temporal queries (time travel)
- Scalable reads via projections
- Clear separation of concerns

**Cons:**
- Complexity overhead for simple use cases
- Event versioning and migration challenges
- Storage overhead (full event history)
- Eventual consistency between write/read models

### 8.7 When to Use

**Good Fit:**
- Audit and compliance requirements
- Complex domain logic
- Need for historical analysis
- Event-driven integrations

**Not Recommended:**
- Simple CRUD applications
- No significant business rules
- Storage constraints (can't keep full history)
- Strong consistency requirements across all queries

### 8.8 Recommendations for Time-Series Platform

**Selective Application:**
- Use event sourcing for core domain aggregates (sensors, configurations)
- Direct time-series database writes for high-volume readings
- CQRS for separating ingestion (write) from analytics (read)

**Hybrid Approach:**
```
High-Frequency Data → Direct to Time-Series DB (no event sourcing)
Configuration Changes → Event Sourcing (full audit trail)
User Actions → Event Sourcing (accountability)
```

**Event Store Options:**
- PostgreSQL with event_sourcing.rs
- Specialized event stores (EventStoreDB)
- Kafka for streaming event log

---

## 9. Observability and Monitoring Architecture

### 9.1 Overview

Modern observability stacks provide:
- **Metrics**: Time-series numerical data
- **Logs**: Textual event records
- **Traces**: Request flow across services
- **Profiles**: Performance characteristics

### 9.2 OpenTelemetry Framework

**Purpose:**
- Vendor-neutral instrumentation
- Standardized data collection
- Prevent vendor lock-in

**Architecture:**
- **SDKs**: Language-specific libraries for instrumentation
- **Collector**: Receives, processes, and exports telemetry
- **Exporters**: Send data to backends (Prometheus, Jaeger, etc.)

**Data Model:**
- Metrics: Gauges, counters, histograms
- Traces: Spans with parent-child relationships
- Logs: Structured with trace correlation
- Resource attributes: Service, host, environment metadata

### 9.3 Prometheus Architecture

**Design:**
- Pull-based metrics collection (scraping)
- Time-series database optimized for metrics
- PromQL query language
- Alerting via Alertmanager

**Data Model:**
```
metric_name{label1="value1", label2="value2"} value timestamp
```

**Components:**
- **Prometheus Server**: Scrapes and stores metrics
- **Exporters**: Expose metrics from various sources
- **Alertmanager**: Handles alerts (deduplication, grouping, routing)
- **Pushgateway**: For short-lived jobs

### 9.4 Grafana Ecosystem

**Core Components:**

1. **Grafana**: Visualization and dashboards
2. **Grafana Alloy**: OpenTelemetry Collector distribution
3. **Grafana Mimir**: Highly scalable Prometheus-compatible backend
4. **Grafana Loki**: Log aggregation system
5. **Grafana Tempo**: Distributed tracing backend

**Integration Patterns:**
```
Application (OTel SDK) → Grafana Alloy (Collector) → Backends
                                                        ↓
                                            ┌───────────┴─────────┐
                                            ↓                     ↓
                                    Prometheus/Mimir         Tempo/Jaeger
                                        (metrics)              (traces)
                                            ↓                     ↓
                                        Grafana (unified visualization)
```

### 9.5 Full Stack Observability

**Components:**
- **Prometheus**: Metrics collection and storage
- **Loki**: Log aggregation (like Prometheus for logs)
- **Tempo**: Distributed tracing
- **Grafana**: Unified visualization and correlation

**Key Features:**
- **Correlation**: Link metrics, logs, and traces
- **Drilldown**: Navigate from metrics to traces to logs
- **Alerting**: Sophisticated rules and notifications
- **Dashboards**: Pre-configured and custom views

### 9.6 Data Flow Patterns

#### **Pull Model (Prometheus)**
```
Prometheus Server → Scrape → /metrics endpoint
                              (exposed by application)
```

**Benefits:**
- Service discovery and dynamic targets
- Prometheus controls scrape interval
- Simplifies application logic

**Drawbacks:**
- Requires network access from Prometheus to targets
- Short-lived jobs need Pushgateway

#### **Push Model (OpenTelemetry → Remote Write)**
```
Application → OTel Collector → Remote Write → Prometheus/Mimir
```

**Benefits:**
- Works behind firewalls
- Multiple collectors can push to same backend
- Better for ephemeral workloads

**Drawbacks:**
- Application must know collector endpoint
- Potential backpressure issues

### 9.7 Observability for Time-Series Platforms

**Metrics to Collect:**
- Ingestion rate (data points/second)
- Query latency (p50, p95, p99)
- Storage utilization
- Processing backlog
- Error rates by source/type

**Example Instrumentation:**
```rust
use opentelemetry::{metrics::*, KeyValue};

let meter = opentelemetry::global::meter("time-series-platform");
let ingestion_counter = meter.u64_counter("data_points_ingested").init();
let query_histogram = meter.f64_histogram("query_duration_seconds").init();

// Record ingestion
ingestion_counter.add(
    batch.len() as u64,
    &[KeyValue::new("source", source_name)]
);

// Record query latency
let start = Instant::now();
let result = execute_query(query).await;
query_histogram.record(
    start.elapsed().as_secs_f64(),
    &[KeyValue::new("query_type", query.query_type())]
);
```

**Tracing Example:**
```rust
use opentelemetry::trace::Tracer;

let tracer = opentelemetry::global::tracer("ingestion");
let mut span = tracer.span_builder("ingest_data").start(&tracer);

span.set_attribute(KeyValue::new("source", source_id));
span.set_attribute(KeyValue::new("batch_size", batch.len() as i64));

let result = ingest_batch(batch).await;

span.end();
```

### 9.8 Recommended Architecture for Time-Series Platform

**Deployment:**
```
Time-Series Platform
    ↓ (instrumented with OTel SDK)
Grafana Alloy (local agent)
    ↓ (exports telemetry)
┌───────────┬──────────────┐
↓           ↓              ↓
Mimir       Loki           Tempo
(metrics)   (logs)         (traces)
    ↓           ↓              ↓
Grafana (unified observability)
```

**Benefits:**
- Self-monitoring: Platform monitors itself using same patterns
- Unified view: Correlate platform behavior with user workloads
- Debugging: Trace requests through ingestion → processing → storage → query
- Performance: Identify bottlenecks via distributed tracing

**Vendor Neutrality:**
- OpenTelemetry ensures portability
- Can switch from Prometheus to Mimir, or Grafana to alternative dashboards
- Avoid lock-in to proprietary solutions

---

## 10. Key Architectural Patterns Summary

### 10.1 Layered Architecture

**Pattern:**
```
┌─────────────────────────────┐
│   Presentation Layer        │  (API, CLI, Web UI)
├─────────────────────────────┤
│   Application Layer         │  (Use Cases, Orchestration)
├─────────────────────────────┤
│   Domain Layer              │  (Business Logic, Entities)
├─────────────────────────────┤
│   Infrastructure Layer      │  (Databases, Message Queues)
└─────────────────────────────┘
```

**Benefits:**
- Separation of concerns
- Clear dependencies (downward only)
- Testability

### 10.2 Hexagonal Architecture (Ports & Adapters)

**Pattern:**
```
       Primary Adapters
         (REST, CLI)
              ↓
         [Driving Ports]
              ↓
         ┌─────────┐
         │ Domain  │
         │  Core   │
         └─────────┘
              ↓
        [Driven Ports]
              ↓
      Secondary Adapters
     (DB, Queue, API)
```

**Benefits:**
- Domain isolation
- Swappable infrastructure
- Testability with mocks

**Application:**
- Domain-agnostic time-series core
- Domain-specific adapters (air quality, finance, IoT)

### 10.3 Event-Driven Architecture

**Pattern:**
```
Event Producer → Event Bus → Event Consumers
                    ↓
               Event Store
```

**Benefits:**
- Loose coupling
- Asynchronous processing
- Scalability

**Application:**
- Sensor data ingestion
- Real-time alerts
- Downstream integrations

### 10.4 CQRS (Command Query Responsibility Segregation)

**Pattern:**
```
Commands → Write Model → Event Store
                              ↓
                         Event Bus
                              ↓
Queries ← Read Models ← Event Handlers
```

**Benefits:**
- Optimized read/write paths
- Scalable reads via projections
- Clear separation of concerns

**Application:**
- High-throughput ingestion (write)
- Complex analytics (read)
- Multiple query patterns

### 10.5 Actor Model

**Pattern:**
```
┌─────────┐    message    ┌─────────┐
│ Actor A │ ─────────────→│ Actor B │
└─────────┘               └─────────┘
    ↓ message                  ↓ message
┌─────────┐               ┌─────────┐
│ Actor C │               │ Actor D │
└─────────┘               └─────────┘
```

**Benefits:**
- Isolated state (no race conditions)
- Message-passing concurrency
- Natural distribution

**Application:**
- Per-sensor actors
- Processing pipeline stages
- Query executors

### 10.6 Microservices

**Pattern:**
```
┌──────────┐  ┌──────────┐  ┌──────────┐
│ Service  │  │ Service  │  │ Service  │
│    A     │  │    B     │  │    C     │
└──────────┘  └──────────┘  └──────────┘
      ↓             ↓             ↓
    [API Gateway / Service Mesh]
```

**Benefits:**
- Independent deployment
- Technology diversity
- Scalability per service

**Considerations:**
- Operational complexity
- Network overhead
- Distributed system challenges

**Application:**
- Ingestion service
- Query service
- Analytics service
- Alert service

### 10.7 Plugin Architecture

**Pattern:**
```
┌───────────────────┐
│   Core Platform   │
└────────┬──────────┘
         │ Plugin API
    ┌────┴─────┬─────────┐
    ↓          ↓         ↓
┌────────┐ ┌────────┐ ┌────────┐
│Plugin A│ │Plugin B│ │Plugin C│
└────────┘ └────────┘ └────────┘
```

**Benefits:**
- Extensibility without core changes
- Community contributions
- Domain-specific customization

**Application:**
- Data source plugins (MQTT, HTTP, Kafka)
- Transformation plugins
- Analysis plugins (ML models)
- Output plugins (alerts, integrations)

### 10.8 Streaming Architecture

**Pattern:**
```
Data Sources → Stream Processor → Sinks
                     ↓
              Stateful Operations
               (windows, joins)
```

**Benefits:**
- Real-time processing
- Backpressure handling
- Fault tolerance

**Application:**
- Real-time anomaly detection
- Aggregation windows
- Stream joins (correlate multiple sensors)

### 10.9 Lambda Architecture

**Pattern:**
```
Data Sources
    ↓
    ├─→ Batch Layer (historical, accurate)
    │        ↓
    │   Batch Views
    │
    └─→ Speed Layer (real-time, approximate)
             ↓
        Real-time Views
             ↓
        Serving Layer (merge views)
```

**Application:**
- Batch: Accurate historical analysis
- Speed: Real-time dashboards
- Serving: Combine for queries spanning both

### 10.10 Time-Series Specific Patterns

**Downsampling:**
```
Raw Data (1s) → 1min aggregates → 1hr aggregates → 1day aggregates
```

**Retention Policies:**
```
Raw data: 7 days
1min data: 30 days
1hr data: 1 year
1day data: forever
```

**Partitioning:**
```
Time-based partitions: 2025-01, 2025-02, ...
  ↓
Efficient range queries and data lifecycle
```

---

## 11. Lessons Learned

### 11.1 From Air Quality Platforms

1. **Modularity is Key**:
   - AirGradient's success: Users can choose their own backend
   - Lesson: Decouple data collection from storage and visualization

2. **Community Matters**:
   - Open-source hardware/software attracts contributors
   - Lesson: Document APIs, provide integration examples

3. **Dual API Strategy**:
   - Local + Cloud APIs serve different use cases
   - Lesson: Support both edge and cloud deployment models

4. **Data Standardization**:
   - OpenAQ's focus on interoperability
   - Lesson: Use standard protocols (InfluxDB line protocol, OTel)

5. **Funding and Sustainability**:
   - 2025 cutbacks to public data programs
   - Lesson: Design for sustainability, consider self-hosted deployments

### 11.2 From Time-Series Databases

1. **Cardinality Management**:
   - InfluxDB struggles with high cardinality
   - Lesson: Design storage to handle many unique time series

2. **Storage Model Matters**:
   - QuestDB's columnar storage outperforms LSM trees for analytics
   - Lesson: Choose storage based on workload (write-heavy vs. query-heavy)

3. **Hybrid Approaches Win**:
   - TimescaleDB's row-columnar hybrid balances ingestion and queries
   - Lesson: Consider tiered storage (hot data row-based, cold data columnar)

4. **Schema Flexibility**:
   - Schema-agnostic ingestion (QuestDB) simplifies integration
   - Lesson: Support dynamic schemas for domain-agnostic platform

5. **Protocol Compatibility**:
   - Supporting InfluxDB protocol increases adoption
   - Lesson: Implement popular protocols for ecosystem compatibility

### 11.3 From Event-Driven Systems

1. **Channels are Powerful**:
   - Flume/Crossbeam provide performant message passing
   - Lesson: Use channels for actor communication and backpressure

2. **Actor Model Simplifies Concurrency**:
   - Actix/Tokio actors avoid race conditions
   - Lesson: Use actors for stateful components (per-sensor actors)

3. **Backpressure is Essential**:
   - Bounded channels prevent memory overflow
   - Lesson: Implement backpressure throughout ingestion pipeline

4. **SEDA Architecture**:
   - Staged event-driven architecture avoids thread overhead
   - Lesson: Pipeline stages with queues enable scalability

### 11.4 From Hexagonal Architecture

1. **Testability Through Isolation**:
   - Domain logic tested independently of infrastructure
   - Lesson: Define ports (traits) for all external dependencies

2. **Swappable Infrastructure**:
   - Easy to switch databases, message queues
   - Lesson: Use adapter pattern for storage, ingestion, output

3. **Clear Boundaries**:
   - Reduces cognitive load, easier to reason about
   - Lesson: Enforce strict layering in code organization

4. **Trade-off: Complexity**:
   - More OOP-oriented, can be verbose in Rust
   - Lesson: Use for complex domains, avoid for simple CRUD

### 11.5 From Plugin Architectures

1. **WebAssembly for Safety**:
   - Sandboxed execution, no unsafe code
   - Lesson: Prefer WASM for user-provided plugins

2. **ABI Instability is Real**:
   - Rust's unstable ABI complicates dynamic loading
   - Lesson: Use WASM or CLI-based plugins to avoid ABI issues

3. **Compile-Time vs. Runtime**:
   - Static linking faster, dynamic linking better for development
   - Lesson: Support both modes (features in Cargo)

4. **Plugin Discovery**:
   - Configuration-based discovery clearer than magic
   - Lesson: Explicit plugin configuration file

### 11.6 From CQRS/Event Sourcing

1. **Audit Trails are Valuable**:
   - Event sourcing provides complete history
   - Lesson: Use for configuration changes, critical operations

2. **Selective Application**:
   - Not every part of system needs event sourcing
   - Lesson: Apply only where audit/temporal queries needed

3. **Event Versioning is Hard**:
   - Event schema evolution requires careful planning
   - Lesson: Version events from day one, plan for upcasting

4. **Eventual Consistency**:
   - Read models lag behind write model
   - Lesson: Design UI to handle eventual consistency

### 11.7 From Observability Platforms

1. **Vendor Neutrality via OpenTelemetry**:
   - Avoid lock-in to proprietary platforms
   - Lesson: Instrument with OTel from the start

2. **Correlation is Critical**:
   - Link metrics, logs, and traces for debugging
   - Lesson: Ensure trace context propagates through system

3. **Self-Monitoring**:
   - Platform should monitor itself
   - Lesson: Expose same metrics/traces as users would

4. **Pull vs. Push**:
   - Both have trade-offs (Prometheus pull, OTel push)
   - Lesson: Support both for flexibility

### 11.8 General Lessons

1. **Start Simple, Add Complexity**:
   - Begin with monolith, extract services when needed
   - Lesson: Avoid premature microservices

2. **Domain Logic is Precious**:
   - Isolate from infrastructure, frameworks
   - Lesson: Hexagonal architecture protects domain

3. **Extensibility from Day One**:
   - Plugin architecture enables community contributions
   - Lesson: Define extension points early

4. **Standards Accelerate Adoption**:
   - OpenTelemetry, InfluxDB protocol, PostgreSQL wire protocol
   - Lesson: Implement industry standards where possible

5. **Observability is Not Optional**:
   - Debugging distributed systems requires tracing
   - Lesson: Instrument thoroughly from the beginning

---

## 12. Best Practices for Extensibility

### 12.1 Design Principles

1. **Open/Closed Principle**:
   - Open for extension, closed for modification
   - Use traits (ports) for extension points

2. **Dependency Inversion**:
   - Depend on abstractions (traits), not concrete implementations
   - Core domain defines interfaces, adapters implement them

3. **Single Responsibility**:
   - Each plugin/module has one reason to change
   - Keep plugins focused

4. **Interface Segregation**:
   - Many specific interfaces better than one general-purpose
   - Example: `DataSourcePlugin`, `TransformPlugin`, `AnalysisPlugin` vs. generic `Plugin`

### 12.2 Plugin API Design

1. **Versioning**:
   ```rust
   pub const PLUGIN_API_VERSION: u32 = 1;

   pub trait Plugin {
       fn api_version(&self) -> u32;
   }
   ```

2. **Capability Discovery**:
   ```rust
   pub trait Plugin {
       fn name(&self) -> &str;
       fn version(&self) -> &str;
       fn capabilities(&self) -> Vec<Capability>;
   }
   ```

3. **Error Handling**:
   ```rust
   pub enum PluginError {
       ConfigurationError(String),
       RuntimeError(String),
       UnsupportedOperation,
   }

   pub type PluginResult<T> = Result<T, PluginError>;
   ```

4. **Configuration Schema**:
   ```rust
   pub trait Plugin {
       fn config_schema(&self) -> JsonSchema;
       fn configure(&mut self, config: Value) -> PluginResult<()>;
   }
   ```

### 12.3 Stability Guarantees

1. **Semantic Versioning**:
   - MAJOR: Breaking changes to plugin API
   - MINOR: Backward-compatible additions
   - PATCH: Bug fixes

2. **Deprecation Policy**:
   ```rust
   #[deprecated(since = "1.2.0", note = "Use `new_method` instead")]
   pub fn old_method(&self) { }
   ```

3. **Feature Flags**:
   ```toml
   [features]
   default = ["stable"]
   stable = []
   experimental = []
   ```

### 12.4 Documentation Requirements

1. **Plugin Developer Guide**:
   - Quick start tutorial
   - API reference
   - Example plugins
   - Testing guidelines

2. **Migration Guides**:
   - How to upgrade between API versions
   - Deprecation timelines

3. **Security Guidelines**:
   - Input validation requirements
   - Resource limits
   - Sandboxing recommendations

### 12.5 Testing Strategies

1. **Plugin Contract Tests**:
   ```rust
   #[test]
   fn test_plugin_implements_api_correctly() {
       let plugin = MyPlugin::new();
       assert_eq!(plugin.api_version(), PLUGIN_API_VERSION);
       // ... test required methods
   }
   ```

2. **Integration Tests**:
   ```rust
   #[tokio::test]
   async fn test_plugin_end_to_end() {
       let platform = Platform::new();
       platform.load_plugin("path/to/plugin.wasm").await.unwrap();
       let result = platform.execute_plugin_action(...).await;
       assert!(result.is_ok());
   }
   ```

3. **Performance Benchmarks**:
   ```rust
   #[bench]
   fn bench_plugin_throughput(b: &mut Bencher) {
       b.iter(|| {
           // Benchmark plugin operation
       });
   }
   ```

### 12.6 Ecosystem Building

1. **Plugin Registry**:
   - Centralized catalog of available plugins
   - Searchable by category, keyword
   - Version compatibility matrix

2. **CLI Tools**:
   ```bash
   platform plugin list
   platform plugin install mqtt-source
   platform plugin update --all
   ```

3. **Template Generators**:
   ```bash
   platform plugin init my-plugin --type=data-source
   # Generates plugin scaffold with tests
   ```

4. **CI/CD Integration**:
   - Automated plugin validation
   - Security scanning
   - Performance regression tests

### 12.7 Monitoring Plugin Health

1. **Metrics**:
   - Plugin load time
   - Execution duration
   - Error rates
   - Resource usage (memory, CPU)

2. **Circuit Breakers**:
   ```rust
   if plugin.error_rate() > threshold {
       platform.disable_plugin(plugin_id);
       alert_admin("Plugin disabled due to high error rate");
   }
   ```

3. **Resource Limits**:
   ```rust
   // WASM sandbox automatically limits resources
   // For native plugins, use cgroups or resource limits
   ```

---

## 13. Trade-offs Analysis

### 13.1 Monolith vs. Microservices

| Aspect | Monolith | Microservices |
|--------|----------|---------------|
| **Development Speed** | Faster initially | Slower setup, faster at scale |
| **Deployment** | Single unit, simple | Complex orchestration |
| **Scaling** | Vertical scaling | Horizontal, per-service scaling |
| **Technology** | Single stack | Polyglot possible |
| **Debugging** | Easier (single process) | Harder (distributed) |
| **Consistency** | ACID transactions | Eventual consistency |

**Recommendation for Time-Series Platform:**
- **Start**: Modular monolith with clear boundaries
- **Scale**: Extract high-throughput services (ingestion, query) when needed
- **Keep Together**: Domain logic, configuration, orchestration

### 13.2 Static vs. Dynamic Typing (Rust)

| Aspect | Compile-Time (Generics) | Runtime (Trait Objects) |
|--------|-------------------------|-------------------------|
| **Performance** | Zero-cost abstractions | Virtual dispatch overhead |
| **Binary Size** | Code duplication (monomorphization) | Single implementation |
| **Flexibility** | Known at compile-time | Can load dynamically |
| **Error Detection** | Compile-time errors | Runtime errors possible |

**Recommendation:**
- **Core Platform**: Generics for performance-critical paths
- **Plugin System**: Trait objects (`Box<dyn Trait>`) for flexibility
- **Hybrid**: Use generics internally, expose trait objects at plugin boundary

### 13.3 Pull vs. Push (Data Ingestion)

| Aspect | Pull (Platform Scrapes) | Push (Source Sends) |
|--------|-------------------------|---------------------|
| **Control** | Platform controls rate | Source controls rate |
| **Firewall** | Platform must access source | Source must access platform |
| **Backpressure** | Natural (slow scrape) | Requires explicit handling |
| **Short-lived Jobs** | Requires Pushgateway | Works naturally |

**Recommendation:**
- **Default**: Push-based (easier for sources behind firewalls)
- **Offer**: Pull-based for Prometheus-compatible sources
- **Hybrid**: Support both via adapters

### 13.4 Schema-Agnostic vs. Schema-Enforced

| Aspect | Schema-Agnostic | Schema-Enforced |
|--------|-----------------|-----------------|
| **Flexibility** | Accept any data shape | Strict validation |
| **Performance** | Query optimization harder | Better query planning |
| **User Experience** | Easy to get started | Upfront configuration |
| **Data Quality** | Validation at query time | Validation at ingestion |

**Recommendation:**
- **Ingestion**: Schema-agnostic (JSON, auto-detection)
- **Storage**: Optional schema definition for optimization
- **Query**: Infer schema or use user-provided hints

### 13.5 Event Sourcing vs. State-Based

| Aspect | Event Sourcing | State-Based |
|--------|----------------|-------------|
| **Audit Trail** | Complete history | Current state only |
| **Time Travel** | Replay events | Not possible |
| **Storage** | All events stored | Latest state only |
| **Complexity** | Event versioning, projections | Simpler model |
| **Query Performance** | Rebuild state (slow) | Direct state access (fast) |

**Recommendation:**
- **Configuration/Metadata**: Event sourcing (audit trail valuable)
- **Time-Series Data**: Direct storage (volume too high for full event history)
- **User Actions**: Event sourcing (accountability)

### 13.6 WebAssembly vs. Native Plugins

| Aspect | WebAssembly | Native (Dynamic Loading) |
|--------|-------------|--------------------------|
| **Safety** | Sandboxed, no unsafe | Requires unsafe FFI |
| **Performance** | ~95% native speed | Native speed |
| **ABI Stability** | Stable | Rust ABI unstable |
| **Language Support** | Any → WASM | Must use Rust ABI |
| **Startup Time** | Faster (smaller binary) | Slower (larger .so) |

**Recommendation:**
- **User Plugins**: WebAssembly (safety, stability)
- **Official Plugins**: Native (performance)
- **Development**: WebAssembly (faster iteration)

### 13.7 Actor Model vs. Shared State

| Aspect | Actor Model | Shared State (Locks) |
|--------|-------------|----------------------|
| **Concurrency** | Message-passing | Lock-based synchronization |
| **Complexity** | Message design | Deadlock avoidance |
| **Performance** | Message overhead | Lock contention |
| **Debugging** | Trace messages | Race conditions hard to debug |
| **Distribution** | Natural | Distributed locks complex |

**Recommendation:**
- **Stateful Components**: Actor model (per-sensor actors)
- **Read-Only Data**: Shared state (Arc<RwLock<>>)
- **High-Frequency Ops**: Lock-free data structures where possible

### 13.8 CQRS vs. Traditional CRUD

| Aspect | CQRS | Traditional CRUD |
|--------|------|------------------|
| **Complexity** | Separate read/write models | Single model |
| **Scalability** | Scale reads independently | Coupled scaling |
| **Consistency** | Eventual consistency | Immediate consistency |
| **Optimization** | Optimize each path | Compromise |

**Recommendation:**
- **High-Throughput Ingestion + Complex Analytics**: CQRS
- **Simple Admin UI**: Traditional CRUD
- **Hybrid**: Use CQRS for data path, CRUD for config

### 13.9 Batch vs. Streaming

| Aspect | Batch Processing | Stream Processing |
|--------|------------------|-------------------|
| **Latency** | High (minutes to hours) | Low (milliseconds to seconds) |
| **Throughput** | Higher (optimize for throughput) | Lower (optimize for latency) |
| **Accuracy** | Exact results | Approximate (windowing) |
| **Complexity** | Simpler | Complex (state management) |

**Recommendation:**
- **Real-Time Alerts**: Streaming
- **Historical Analysis**: Batch
- **Dashboards**: Streaming with approximate results
- **Reports**: Batch with exact results

### 13.10 Cloud vs. Edge

| Aspect | Cloud-Hosted | Edge-Deployed |
|--------|--------------|---------------|
| **Latency** | Higher (network hop) | Lower (local) |
| **Resources** | Abundant | Constrained |
| **Cost** | Ongoing cloud fees | Upfront hardware |
| **Privacy** | Data leaves network | Data stays local |
| **Maintenance** | Provider manages | User manages |

**Recommendation:**
- **Architecture**: Support both deployments
- **Default**: Cloud for easy getting started
- **Option**: Edge for privacy-sensitive or low-latency needs
- **Hybrid**: Edge preprocessing, cloud storage/analytics

---

## 14. Recommendations

### 14.1 Core Architecture

**Recommendation: Hexagonal Architecture with Actor-Based Concurrency**

**Rationale:**
- Hexagonal architecture provides domain isolation and testability
- Actor model simplifies concurrent data ingestion and processing
- Combination allows domain-agnostic core with domain-specific adapters

**Structure:**
```
platform-core/           # Domain logic (ports)
  ├── entities/          # Time-series data models
  ├── services/          # Business logic
  └── ports/             # Trait definitions
platform-adapters/       # Infrastructure (adapters)
  ├── sources/           # Data source adapters
  ├── storage/           # Storage adapters
  ├── analysis/          # Analysis adapters
  └── output/            # Output adapters
platform-runtime/        # Actor runtime and orchestration
platform-api/            # REST/gRPC API
platform-cli/            # Command-line interface
```

### 14.2 Data Ingestion

**Recommendation: Push-Based with Backpressure**

**Design:**
- Accept data via HTTP POST, gRPC, MQTT
- Tokio channels with bounded capacity
- Backpressure signals when capacity reached

**Pipeline:**
```
Data Source → HTTP Handler → Ingestion Actor → Transform Actor → Storage Actor
               ↓               ↓                  ↓                ↓
            Validate       Enqueue            Process         Persist
```

### 14.3 Storage Strategy

**Recommendation: Pluggable Storage with Default**

**Default**: QuestDB
- Fastest ingestion performance
- Schema-agnostic (domain-agnostic platform)
- Excellent high-cardinality support
- InfluxDB and PostgreSQL protocol support

**Pluggable Via Adapters**:
- TimescaleDB (PostgreSQL users)
- InfluxDB (existing InfluxDB users)
- Custom time-series databases

**Implementation:**
```rust
pub trait TimeSeriesStorage {
    async fn write(&self, points: Vec<DataPoint>) -> Result<(), Error>;
    async fn query(&self, query: Query) -> Result<DataFrame, Error>;
}

pub struct QuestDBStorage { /* ... */ }
pub struct TimescaleDBStorage { /* ... */ }
pub struct InfluxDBStorage { /* ... */ }
```

### 14.4 Event-Driven Architecture

**Recommendation: Actor Model with Tokio/Actix**

**Actors:**
1. **Ingestion Actors**: One per data source, handle incoming data
2. **Transform Actors**: Apply domain-specific transformations
3. **Storage Actors**: Write to time-series database
4. **Query Actors**: Execute queries and return results
5. **Alert Actors**: Monitor data streams for conditions

**Message Passing**: Tokio channels (async) or Actix (if more complex)

**Example:**
```rust
struct IngestionActor {
    source_id: String,
    transform_tx: mpsc::Sender<TransformMessage>,
}

impl IngestionActor {
    async fn run(mut self, mut rx: mpsc::Receiver<IngestMessage>) {
        while let Some(msg) = rx.recv().await {
            let transformed = self.transform(msg.data);
            self.transform_tx.send(transformed).await;
        }
    }
}
```

### 14.5 Extensibility via Plugins

**Recommendation: WebAssembly Plugins with CLI Fallback**

**Plugin Types:**
- **Data Sources**: Ingest from custom protocols
- **Transformations**: Domain-specific data normalization
- **Analysis**: Custom algorithms, ML models
- **Outputs**: Custom alerts, integrations

**WebAssembly Advantages:**
- Safe sandboxed execution
- No ABI stability issues
- Language-agnostic (compile to WASM)

**CLI Fallback:**
- For plugins needing OS access or heavy native dependencies
- Communicate via stdin/stdout JSON

**Plugin Discovery:**
```toml
# platform.toml
[plugins]
mqtt-source = { path = "plugins/mqtt_source.wasm" }
air-quality-transform = { path = "plugins/air_quality.wasm" }
```

### 14.6 Domain-Agnostic Design

**Recommendation: Generic Data Model with Domain Extensions**

**Core Data Model:**
```rust
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub metric: String,
    pub value: Value,  // f64, i64, bool, string
    pub tags: HashMap<String, String>,
}
```

**Domain-Specific Extensions via Adapters:**
- Air Quality Adapter: Parses `pm25`, `co2` metrics, adds `location` tag
- Financial Adapter: Parses `price`, `volume` metrics, adds `symbol` tag
- IoT Adapter: Parses sensor readings, adds `device_id` tag

**Benefit**: Core platform remains domain-agnostic, adapters add domain knowledge

### 14.7 API Design

**Recommendation: Multi-Protocol Support**

**Protocols:**
1. **REST API**: Broad compatibility, easy to use
2. **gRPC**: High performance, streaming support
3. **GraphQL**: Flexible queries, client-driven
4. **WebSocket**: Real-time data streaming

**API Versioning:**
```
/api/v1/ingest
/api/v1/query
/api/v2/ingest  (when breaking changes needed)
```

**Authentication:**
- API tokens for programmatic access
- OAuth2 for user authentication
- mTLS for service-to-service

### 14.8 Observability

**Recommendation: OpenTelemetry with Prometheus & Grafana**

**Instrumentation:**
- Metrics: Ingestion rate, query latency, error rates
- Traces: Request flow through ingestion → processing → storage → query
- Logs: Structured logging with trace correlation

**Stack:**
- OpenTelemetry SDK for instrumentation
- Grafana Alloy (OTel Collector) for data collection
- Prometheus/Mimir for metrics storage
- Tempo for traces storage
- Grafana for visualization

**Self-Monitoring:**
- Platform monitors itself using same patterns as user data
- Dogfooding ensures observability works

### 14.9 CQRS for Data Path

**Recommendation: Separate Ingestion and Query Paths**

**Write Path (Commands):**
```
Ingest API → Validation → Ingestion Actor → Storage
```

**Read Path (Queries):**
```
Query API → Query Planner → Query Executors → Result Aggregation
```

**Benefits:**
- Optimize ingestion for throughput
- Optimize queries for latency and flexibility
- Scale independently

**Implementation:**
- Shared storage (time-series database)
- Separate read models for common queries (materialized views)

### 14.10 Deployment Options

**Recommendation: Container-First with Edge Support**

**Deployment Modes:**
1. **Cloud**: Kubernetes deployment, managed service
2. **Edge**: Single-binary with embedded database
3. **Hybrid**: Edge preprocessing, cloud storage

**Container:**
```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/platform /usr/local/bin/
CMD ["platform", "serve"]
```

**Edge Binary:**
- Embedded QuestDB or SQLite
- Minimal dependencies
- Single configuration file

### 14.11 Testing Strategy

**Recommendation: Multi-Level Testing**

1. **Unit Tests**: Domain logic, no dependencies
   ```rust
   #[test]
   fn test_data_point_validation() {
       let point = DataPoint::new(...);
       assert!(point.is_valid());
   }
   ```

2. **Integration Tests**: Actors, storage, APIs
   ```rust
   #[tokio::test]
   async fn test_ingestion_pipeline() {
       let platform = TestPlatform::new().await;
       platform.ingest(data).await;
       let result = platform.query(...).await;
       assert_eq!(result, expected);
   }
   ```

3. **Contract Tests**: Plugin API stability
   ```rust
   #[test]
   fn test_plugin_contract() {
       let plugin = load_plugin("test_plugin.wasm");
       assert_eq!(plugin.api_version(), CURRENT_API_VERSION);
   }
   ```

4. **Performance Tests**: Benchmarks, load testing
   ```rust
   #[bench]
   fn bench_ingestion_throughput(b: &mut Bencher) {
       b.iter(|| ingest_batch(test_data()));
   }
   ```

5. **End-to-End Tests**: Full stack, realistic scenarios
   ```bash
   # docker-compose up test environment
   # Run automated tests against deployed platform
   ```

### 14.12 Roadmap Approach

**Phase 1: MVP (Monolith)**
- Core data model
- Single storage backend (QuestDB)
- REST API for ingest/query
- Basic observability

**Phase 2: Extensibility**
- Plugin system (WebAssembly)
- Multiple storage adapters
- Actor-based concurrency

**Phase 3: Scale**
- CQRS for ingestion/query separation
- Horizontal scaling (multiple instances)
- Advanced analytics (ML integration)

**Phase 4: Ecosystem**
- Plugin marketplace
- Cloud-hosted offering
- Community contributions

---

## 15. Sources

### Open-Source Air Quality Monitoring
- [AirGradient Air Quality Monitors](https://www.airgradient.com/)
- [OpenAQ Homepage](https://openaq.org/)
- [IEEE: Integrated Open Source Indoor Air Quality Monitoring Platform](https://ieeexplore.ieee.org/document/10734643/)
- [GAIA A08 - Open Source Air Quality Monitor](https://aqicn.org/gaia/a08/)
- [EnviroMonitor - Community Air Quality Monitoring](https://enviromonitor.github.io/)
- [Aquality32: Low-cost Open-Source Air Quality Device](https://www.sciencedirect.com/science/article/pii/S2468067224001019)

### AirGradient Software Stack
- [AirGradient API Documentation](https://www.airgradient.com/air-quality-monitoring-toolkit/operating/airgradient-api/)
- [Dashboard API Documentation - AirGradient Forum](https://forum.airgradient.com/t/dashboard-api-documentation/250)
- [AirGradient - Bindings | openHAB](https://www.openhab.org/addons/bindings/airgradient/)
- [GitHub - airgradienthq/airgradient-map-api](https://github.com/airgradienthq/airgradient-map-api)
- [GitHub - airgradienthq/arduino](https://github.com/airgradienthq/arduino)
- [Jeff Geerling: Monitoring Air Quality with AirGradient](https://www.jeffgeerling.com/blog/2021/airgradient-diy-air-quality-monitor-co2-pm25)

### Time-Series Intelligence Platforms
- [Cisco Data Fabric Transforms Machine Data into AI-Ready Intelligence](https://newsroom.cisco.com/c/r/newsroom/en/us/a/y2025/m09/cisco-data-fabric-transforms-machine-data-into-ai-ready-intelligence.html)
- [Navigating the AIOps Platform Landscape in 2025](https://cloudchipr.com/blog/aiops-platform)
- [Deep Learning for Time Series Forecasting Survey](https://link.springer.com/article/10.1007/s10462-025-11223-9)
- [Modern Data Platform Architecture in 2025](https://www.domo.com/learn/article/how-to-architect-a-modern-data-platform-in-2025)
- [C3 AI: Time Series Modeling Redefined](https://c3.ai/blog/time-series-modeling-redefined-a-breakthrough-approach/)

### Rust Event-Driven Architecture
- [Asynchronous Design Patterns in Rust](https://www.linkedin.com/pulse/asynchronous-design-patterns-inrust-luis-soares-m-sc--4mskf)
- [GitHub - actix/actix: Actor Framework for Rust](https://github.com/actix/actix)
- [Alice Ryhl: Actors with Tokio](https://ryhl.io/blog/actors-with-tokio/)
- [Sling Academy: Actor-Based Concurrency in Rust with Actix](https://www.slingacademy.com/article/actor-based-concurrency-in-rust-introducing-the-actix-ecosystem/)
- [Ferdinand de Antoni: Tiny Tokio Actors](https://fdeantoni.medium.com/tiny-tokio-actors-3a2ec958ef43)
- [Denis Kolodin: Designing an Abstract Actor Task](https://medium.com/knwldev/designing-an-abstract-actor-task-implementing-actors-framework-part-i-ab4f7b22aec3)

### Hexagonal Architecture in Rust
- [Luca Corsetti: Hexagonal Architecture in Rust](https://medium.com/@lucorset/hexagonal-architecture-in-rust-72f8958eb26d)
- [Barrage: How to Apply Hexagonal Architecture to Rust](https://www.barrage.net/blog/technology/how-to-apply-hexagonal-architecture-to-rust)
- [GitHub - antoinecarton/hexagonal-rust](https://github.com/antoinecarton/hexagonal-rust)
- [Software Design Patterns in Rust - Chapter 38](https://sdpr.rantai.dev/docs/part-vi/chapter-38/)
- [How to Code It: Master Hexagonal Architecture in Rust](https://www.howtocodeit.com/articles/master-hexagonal-architecture-rust)
- [Tuttlem: Hexagonal Architecture in Rust](http://tuttlem.github.io/2025/08/31/hexagonal-architecture-in-rust.html)
- [Alistair Cockburn: Hexagonal Architecture](https://alistair.cockburn.us/hexagonal-architecture)

### Message Bus & Channels
- [Leapcell: Building Robust Concurrent Pipelines with Crossbeam and Flume](https://leapcell.io/blog/building-robust-concurrent-pipelines-with-crossbeam-and-flume-channels-in-rust)
- [Flume - Rust Concurrency Library](https://lib.rs/crates/flume)
- [GitHub - resolvingarchitecture/seda-bus](https://github.com/resolvingarchitecture/seda-bus)
- [Gregory Terzian: Rust Concurrency Patterns](https://medium.com/@polyglot_factotum/rust-concurrency-patterns-communicate-by-sharing-your-sender-re-visited-9d42e6dfecfa)
- [GitHub - zesterer/flume](https://github.com/zesterer/flume)
- [GitHub - crossbeam-rs/crossbeam](https://github.com/crossbeam-rs/crossbeam)

### Time-Series Databases
- [QuestDB: Comparing InfluxDB, TimescaleDB, and QuestDB](https://questdb.com/blog/comparing-influxdb-timescaledb-questdb-time-series-databases/)
- [RisingWave: QuestDB vs TimescaleDB vs InfluxDB](https://risingwave.com/blog/questdb-vs-timescaledb-vs-influxdb-choosing-the-best-for-time-series-data-processing/)
- [RisingWave: Performance and Scalability Comparison](https://risingwave.com/blog/performance-and-scalability-of-influxdb-timescaledb-and-questdb/)
- [QuestDB: TimescaleDB vs. QuestDB Performance Benchmarks](https://questdb.com/blog/timescaledb-vs-questdb-comparison/)
- [InfoQ: High Performance Time-Series Database Design with QuestDB](https://www.infoq.com/presentations/questdb/)
- [QuestDB: Benchmark vs. InfluxDB](https://questdb.com/blog/2024/02/26/questdb-versus-influxdb/)

### Plugin Architecture
- [Michael F. Bryan: Plugins in Rust](https://adventures.michaelfbryan.com/posts/plugins-in-rust/)
- [Tuttlem: Loading Dynamic Libraries in Rust](https://tuttlem.github.io/2025/11/15/loading-dynamic-libraries-in-rust.html)
- [NullDeref: Plugins in Rust - The Technologies](https://nullderef.com/blog/plugin-tech/)
- [PeerDH: Implementing Rust-based Plugin Architecture](https://peerdh.com/blogs/programming-insights/implementing-a-rust-based-plugin-architecture-for-dynamic-feature-loading)
- [DEV Community: Plugin-Based Architecture in Rust](https://dev.to/mineichen/plugin-based-architecture-in-rust-4om7)
- [Zicklag: Rust Plugins Tutorial](https://zicklag.github.io/rust-tutorials/rust-plugins.html)
- [NullDeref: Plugins in Rust - Dynamic Loading](https://nullderef.com/blog/plugin-dynload/)

### CQRS and Event Sourcing
- [CQRS and Event Sourcing Using Rust](https://doc.rust-cqrs.org/)
- [katayama8000: Building API with Rust, CQRS, and Event Sourcing](https://medium.com/@tattu.0310/building-an-api-with-rust-cqrs-and-event-sourcing-09a702bf8bc5)
- [cqrs-es Rust Documentation](https://docs.rs/cqrs-es)
- [GitHub - primait/event_sourcing.rs](https://github.com/primait/event_sourcing.rs)
- [GitHub - socgnachilderic/rust_ddd-cqrs-es](https://github.com/socgnachilderic/rust_ddd-cqrs-es)
- [GitHub - serverlesstechnology/cqrs](https://github.com/serverlesstechnology/cqrs)
- [GitHub - fraktalio/fmodel-rust](https://github.com/fraktalio/fmodel-rust)

### Observability
- [Grafana: The Open Observability Platform](https://grafana.com/)
- [Grafana Cloud: Application Observability](https://grafana.com/docs/grafana-cloud/monitor-applications/application-observability/)
- [Monitoring Framework: Integrate OpenTelemetry with Prometheus and Grafana](https://monitoringframework.com/pagina/integrate-opentelemetry-with-prometheus-and-grafana)
- [Grafana Blog: Practical Guide to OpenTelemetry and Prometheus](https://grafana.com/blog/2023/07/20/a-practical-guide-to-data-collection-with-opentelemetry-and-prometheus/)
- [Grafana Blog: Application Observability with OpenTelemetry](https://grafana.com/blog/2023/11/14/announcing-application-observability-in-grafana-cloud-with-native-support-for-opentelemetry-and-prometheus/)
- [Last9: Integrating OpenTelemetry with Grafana](https://last9.io/blog/opentelemetry-with-grafana/)
- [Microsoft: OpenTelemetry with Prometheus, Grafana, Jaeger](https://learn.microsoft.com/en-us/dotnet/core/diagnostics/observability-prgrja-example)
- [Venkata Ashok: Full Stack Observability](https://medium.com/@venkat65534/full-stack-observability-with-grafana-prometheus-loki-tempo-and-opentelemetry-90839113d17d)

---

**End of Research Document**

**Next Steps:**
1. Review findings with stakeholders
2. Validate architectural decisions
3. Create detailed design specifications
4. Begin implementation of MVP (Phase 1)

**Document Version:** 1.0
**Last Updated:** 2025-12-13
