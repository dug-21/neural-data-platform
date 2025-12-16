# AIR-005 IngestionCoordinator Architecture Diagrams

## Component Interaction Diagram

```mermaid
graph TD
    subgraph "air-quality-app Container"
        Main[main.rs]

        subgraph "IngestionCoordinator"
            Coord[Coordinator]
            MasterTx[Master Sender]
            MasterRx[Master Receiver]
        end

        subgraph "SourceManager"
            SM[SourceManager]
            Watch[Registry Watcher]
            Factories[Factory Registry]
        end

        subgraph "Sources"
            MQTT[MQTT Source Task]
            HTTP1[HTTP Weather Task]
            HTTP2[HTTP AirQual Task]
        end

        subgraph "Routing"
            Router[IngestionRouter]
            DLQ[Dead Letter Queue]
        end

        subgraph "Storage"
            SW1[StorageWriter 1]
            SW2[StorageWriter 2]
            SW3[StorageWriter 3]
            Store[ParquetStore]
        end
    end

    subgraph "External"
        etcd[(etcd)]
        MQTTBroker[MQTT Broker]
        OpenWeather[OpenWeather API]
    end

    Main -->|creates| Coord
    Coord -->|owns| MasterTx
    Coord -->|owns| MasterRx
    Coord -->|creates| SM
    Coord -->|creates| Router

    SM -->|registers| Factories
    SM -->|spawns| MQTT
    SM -->|spawns| HTTP1
    SM -->|spawns| HTTP2
    SM -->|monitors| Watch

    Watch -->|watches| etcd
    Factories -->|creates| MQTT
    Factories -->|creates| HTTP1

    MQTT -->|send(point)| MasterTx
    HTTP1 -->|send(point)| MasterTx
    HTTP2 -->|send(point)| MasterTx

    MasterRx -->|recv()| Router
    Router -->|validate & route| SW1
    Router -->|validate & route| SW2
    Router -->|validate & route| SW3
    Router -->|invalid data| DLQ

    SW1 -->|write_batch| Store
    SW2 -->|write_batch| Store
    SW3 -->|write_batch| Store

    MQTT -.->|poll| MQTTBroker
    HTTP1 -.->|HTTP GET| OpenWeather
    HTTP2 -.->|HTTP GET| OpenWeather

    classDef coordinator fill:#e1f5ff,stroke:#0066cc
    classDef sources fill:#fff4e1,stroke:#cc8800
    classDef routing fill:#e8f5e9,stroke:#00aa44
    classDef storage fill:#f3e5f5,stroke:#8800cc

    class Coord,MasterTx,MasterRx coordinator
    class SM,Watch,Factories,MQTT,HTTP1,HTTP2 sources
    class Router,DLQ routing
    class SW1,SW2,SW3,Store storage
```

## Data Flow Sequence

```mermaid
sequenceDiagram
    participant Main
    participant Coordinator
    participant SourceManager
    participant Factory
    participant MQTTTask
    participant Channel
    participant Router
    participant Storage

    Main->>Coordinator: new(registry, 10000)
    Coordinator->>Coordinator: create master channel
    Coordinator->>SourceManager: new(sender.clone())
    Coordinator->>Router: new(registry)

    Main->>Coordinator: start()
    Coordinator->>SourceManager: start(cancel_token)

    SourceManager->>Factory: spawn("air-quality", config, sender, token)
    Factory->>MQTTTask: tokio::spawn(mqtt_loop)
    MQTTTask-->>SourceManager: JoinHandle

    SourceManager->>SourceManager: store handle in active_sources

    Coordinator->>Router: spawn router task

    Note over MQTTTask,Channel: Data Production

    loop Every message
        MQTTTask->>MQTTTask: receive from broker
        MQTTTask->>MQTTTask: parse into TimeSeriesPoint
        MQTTTask->>MQTTTask: add stream_id tag
        MQTTTask->>Channel: sender.send(point)
    end

    Note over Router,Storage: Data Consumption

    loop Until channel closes
        Router->>Channel: receiver.recv()
        Router->>Router: validate against schema
        Router->>Router: route by stream_id
        Router->>Storage: storage_tx.send(point)
    end

    Note over Main,Storage: Shutdown Sequence

    Main->>Coordinator: shutdown()
    Coordinator->>Coordinator: cancel_token.cancel()
    Coordinator->>SourceManager: stop_all()
    SourceManager->>MQTTTask: cancel task
    SourceManager->>MQTTTask: await join handle
    MQTTTask-->>SourceManager: Ok(())
    Coordinator->>Coordinator: drop(sender)
    Note over Channel: Channel closes
    Router->>Channel: recv() returns None
    Router->>Router: flush remaining data
    Router-->>Coordinator: Ok(())
    Coordinator-->>Main: Ok(())
```

## Channel Ownership Model

```mermaid
graph LR
    subgraph "IngestionCoordinator (Owner)"
        Tx[master_tx: Sender<br/>OWNED]
        Rx[master_rx: Receiver<br/>OWNED → MOVED]
    end

    subgraph "SourceManager"
        SMTx[sender: Sender<br/>CLONED]
    end

    subgraph "Source Tasks"
        S1Tx[sender: Sender<br/>CLONED]
        S2Tx[sender: Sender<br/>CLONED]
        S3Tx[sender: Sender<br/>CLONED]
    end

    subgraph "IngestionRouter"
        RxOwned[receiver: Receiver<br/>OWNED]
        StorageMap[HashMap<<br/>stream_id,<br/>Sender<br/>>]
    end

    subgraph "Storage Writers"
        ST1[storage_rx1<br/>OWNED]
        ST2[storage_rx2<br/>OWNED]
        ST3[storage_rx3<br/>OWNED]
    end

    Tx -->|clone()| SMTx
    SMTx -->|clone()| S1Tx
    SMTx -->|clone()| S2Tx
    SMTx -->|clone()| S3Tx

    Rx -->|move()| RxOwned

    StorageMap -->|reference| ST1
    StorageMap -->|reference| ST2
    StorageMap -->|reference| ST3

    S1Tx -.->|send()| RxOwned
    S2Tx -.->|send()| RxOwned
    S3Tx -.->|send()| RxOwned

    RxOwned -.->|send()| ST1
    RxOwned -.->|send()| ST2
    RxOwned -.->|send()| ST3

    classDef owned fill:#ffcccc,stroke:#cc0000
    classDef cloned fill:#ccffcc,stroke:#00cc00
    classDef moved fill:#ccccff,stroke:#0000cc

    class Tx,Rx,RxOwned,ST1,ST2,ST3 owned
    class SMTx,S1Tx,S2Tx,S3Tx cloned
```

## Shutdown Coordination

```mermaid
stateDiagram-v2
    [*] --> Running: start()

    Running --> ShuttingDown: shutdown() called

    state ShuttingDown {
        [*] --> CancelToken: cancel global token
        CancelToken --> StopSources: notify all tasks

        state StopSources {
            [*] --> StoppingSource1
            [*] --> StoppingSource2
            [*] --> StoppingSource3

            StoppingSource1 --> Source1Stopped: cancel + await
            StoppingSource2 --> Source2Stopped: cancel + await
            StoppingSource3 --> Source3Stopped: cancel + await

            Source1Stopped --> [*]
            Source2Stopped --> [*]
            Source3Stopped --> [*]
        }

        StopSources --> CloseMasterChannel: all sources stopped
        CloseMasterChannel --> WaitRouter: drop(sender)
        WaitRouter --> Completed: router.await()
    }

    Completed --> [*]

    note right of CancelToken
        Timeout: 5s per source
        Total: 30s max
    end note
```

## Error Handling Flow

```mermaid
flowchart TD
    Start[Source produces error] --> Classify{Classify Error}

    Classify -->|Transient| Retry[Retry with backoff]
    Classify -->|Permanent| Log[Log and skip]
    Classify -->|RateLimit| Wait[Wait + Retry]

    Retry --> CheckRetries{Retries < Max?}
    CheckRetries -->|Yes| Retry
    CheckRetries -->|No| Circuit[Open Circuit Breaker]

    Circuit --> Cooldown[Wait 60s cooldown]
    Cooldown --> HalfOpen[Half-Open State]
    HalfOpen --> TestRetry[Test Retry]
    TestRetry --> Success{Success?}

    Success -->|Yes| Close[Close Circuit]
    Success -->|No| Circuit

    Close --> Running[Resume Normal Operation]
    Log --> Running
    Wait --> Retry

    style Circuit fill:#ff9999
    style Close fill:#99ff99
    style Running fill:#99ccff
```

## Component State Machine

```mermaid
stateDiagram-v2
    [*] --> Created: new()
    Created --> Starting: start()

    state Starting {
        [*] --> SpawningWatcher
        SpawningWatcher --> LoadingConfigs
        LoadingConfigs --> SpawningSources
        SpawningSources --> SpawningRouter
    }

    Starting --> Running: all tasks spawned

    state Running {
        [*] --> Monitoring
        Monitoring --> ConfigChange: etcd watch event
        ConfigChange --> AddSource: new stream
        ConfigChange --> RemoveSource: disabled stream
        ConfigChange --> UpdateSource: config changed

        AddSource --> Monitoring
        RemoveSource --> Monitoring
        UpdateSource --> Monitoring

        Monitoring --> SourceCrash: task panic
        SourceCrash --> Restarting: circuit closed
        SourceCrash --> Monitoring: circuit open
        Restarting --> Monitoring: restart success
    }

    Running --> ShuttingDown: shutdown()

    state ShuttingDown {
        [*] --> StoppingWatcher
        StoppingWatcher --> StoppingSources
        StoppingSources --> StoppingRouter
        StoppingRouter --> Cleanup
    }

    ShuttingDown --> [*]: shutdown complete
```

## Factory Pattern Implementation

```mermaid
classDiagram
    class SourceFactory {
        <<trait>>
        +spawn(stream_id, config, sender, token) JoinHandle
        +name() &str
    }

    class MqttSourceFactory {
        +new() Self
        +spawn() JoinHandle
        +name() &str
    }

    class HttpPollSourceFactory {
        +new() Self
        +spawn() JoinHandle
        +name() &str
    }

    class WebSocketSourceFactory {
        +new() Self
        +spawn() JoinHandle
        +name() &str
    }

    class SourceManager {
        -factories: HashMap~String, Arc~dyn SourceFactory~~
        -active_sources: HashMap~String, SourceHandle~
        +register_factory(type, factory)
        +spawn_source(stream_id)
        +stop_source(stream_id)
    }

    class SourceHandle {
        +stream_id: String
        +source_type: String
        +cancel_token: CancellationToken
        +task_handle: JoinHandle
        +stop() Result
    }

    SourceFactory <|.. MqttSourceFactory: implements
    SourceFactory <|.. HttpPollSourceFactory: implements
    SourceFactory <|.. WebSocketSourceFactory: implements

    SourceManager --> SourceFactory: uses
    SourceManager --> SourceHandle: manages

    note for SourceFactory "Abstract factory pattern allows\ndynamic source type registration"
    note for SourceHandle "Encapsulates task lifecycle\nfor graceful shutdown"
```

## Memory Layout

```
┌────────────────────────────────────────────────────────────┐
│  Memory Region: IngestionCoordinator                       │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  Stack (Function Scope):                                   │
│    coordinator: IngestionCoordinator    ~1 KB             │
│                                                            │
│  Heap (Owned Data):                                        │
│    ┌─────────────────────────────────────────────────┐   │
│    │  Master Channel Buffer                          │   │
│    │  - Capacity: 10,000 items                       │   │
│    │  - Item size: ~200 bytes (TimeSeriesPoint)     │   │
│    │  - Total: ~2 MB                                 │   │
│    └─────────────────────────────────────────────────┘   │
│                                                            │
│    ┌─────────────────────────────────────────────────┐   │
│    │  SourceManager                                  │   │
│    │  - active_sources: HashMap                      │   │
│    │    - 3 entries × 100 bytes = 300 bytes         │   │
│    │  - factories: HashMap                           │   │
│    │    - 3 entries × 100 bytes = 300 bytes         │   │
│    │  - Total: ~5 KB                                 │   │
│    └─────────────────────────────────────────────────┘   │
│                                                            │
│    ┌─────────────────────────────────────────────────┐   │
│    │  IngestionRouter                                │   │
│    │  - storage_channels: HashMap                    │   │
│    │    - 3 entries × 50 bytes = 150 bytes          │   │
│    │  - dead_letter_tx: Sender                       │   │
│    │  - Total: ~10 KB                                │   │
│    └─────────────────────────────────────────────────┘   │
│                                                            │
│  Arc-Shared Data:                                          │
│    ┌─────────────────────────────────────────────────┐   │
│    │  StreamRegistry (shared with config-client)     │   │
│    │  - Config cache                                  │   │
│    │  - etcd client                                   │   │
│    │  - Total: ~50 KB (shared, not counted twice)   │   │
│    └─────────────────────────────────────────────────┘   │
│                                                            │
├────────────────────────────────────────────────────────────┤
│  TOTAL COORDINATOR OVERHEAD: ~2.02 MB                      │
└────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│  Memory Region: Source Tasks (3x)                          │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  MQTT Source:                                              │
│    - rumqttc client buffers: ~100 KB                      │
│    - cached_points Vec: ~10 KB                            │
│    - Total: ~110 KB                                        │
│                                                            │
│  HTTP Weather Source:                                      │
│    - reqwest client: ~50 KB                               │
│    - response buffers: ~5 KB                              │
│    - Total: ~55 KB                                         │
│                                                            │
│  HTTP AirQual Source:                                      │
│    - reqwest client: ~50 KB                               │
│    - response buffers: ~5 KB                              │
│    - Total: ~55 KB                                         │
│                                                            │
├────────────────────────────────────────────────────────────┤
│  TOTAL SOURCES OVERHEAD: ~220 KB                           │
└────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│  Grand Total: ~2.24 MB                                     │
│  Container Limit: 512 MB                                   │
│  Coordinator Usage: 0.44% of limit                         │
└────────────────────────────────────────────────────────────┘
```

---

## Key Metrics

### Throughput
- **Master Channel**: 10,000 points buffer → ~50,000 points/sec throughput
- **Per-Stream Channels**: 1,000 points buffer → ~5,000 points/sec per stream
- **Bottleneck**: Storage writer batching (100 points, 5s timeout)

### Latency
- **Source → Channel**: < 1ms (in-memory send)
- **Channel → Router**: < 1ms (recv + validate)
- **Router → Storage**: < 1ms (send to writer)
- **End-to-End**: ~10-50ms (depends on storage batch timing)

### Resource Usage
- **CPU**: ~2-5% (idle), ~10-20% (active ingestion)
- **Memory**: ~2.5 MB (coordinator) + ~220 KB (sources) = ~2.7 MB total
- **Network**: ~1 KB/point × 100 points/sec = ~100 KB/sec

---

## Design Principles Applied

1. **Single Responsibility**: Each component has one clear purpose
2. **Dependency Inversion**: Depend on traits, not concrete types
3. **Open/Closed**: Extensible via factories without modifying core
4. **Interface Segregation**: Small, focused trait interfaces
5. **Composition over Inheritance**: Components composed, not inherited

---

## Related Documents

- [Full Design Document](../AIR-005_INGESTION_COORDINATOR_DESIGN.md)
- [SPARC Architecture](../../../product/features/air-005/architecture/ARCHITECTURE.md)
- [StreamConfig Schema](../../../core/src/types/stream_config.rs)
