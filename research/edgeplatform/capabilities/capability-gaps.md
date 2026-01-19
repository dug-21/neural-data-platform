# Edge Data Platform Capability Gaps and Needed Features

**Research Date:** January 2025
**Platform Context:** Raspberry Pi (<2GB RAM), Rust-based, current capabilities include MQTT/HTTP ingestion, Parquet (Bronze) and TimescaleDB (Silver) storage, configuration-driven ETL, data quality rules, MCP integration, GitOps via etcd

---

## Executive Summary

This research identifies capability gaps that would transform the Neural Data Platform from a capable IoT data platform into a comprehensive edge computing solution. The analysis covers protocol support, edge ML/AI, real-time processing, edge-cloud sync, security, visualization, inter-node communication, and hardware acceleration.

The edge computing market is experiencing explosive growth, from USD 21.19 billion in 2025 to an expected USD 44.73 billion by 2030 (16.1% CAGR). Industrial IoT constitutes 33.3% of this market, with manufacturers reporting 89% intent to shift AI inference to local gateways for real-time quality control. This validates the market opportunity for a complete edge data platform.

---

## 1. Protocol Support

### Current State
- MQTT (pub/sub messaging)
- HTTP polling

### Gap Analysis

| Protocol | Use Case | Market Demand | Rust Support | Priority |
|----------|----------|---------------|--------------|----------|
| **OPC-UA** | Industrial automation, SCADA | Critical for IIoT | Good (async-opcua, locka99/opcua) | HIGH |
| **Modbus RTU/TCP** | Legacy industrial devices | High for brownfield | Excellent (tokio-modbus, modbus-core) | HIGH |
| **CAN Bus** | Automotive, industrial machinery | Growing | Good (embedded-can, zencan) | MEDIUM |
| **BLE** | Sensors, wearables | High for consumer IoT | Good (btleplug, esp32-nimble) | MEDIUM |
| **LoRaWAN** | Long-range low-power sensors | High for outdoor/agriculture | Good (rust-lorawan, drogue-device) | MEDIUM |
| **Zigbee/Thread** | Smart home, building automation | Growing (Matter adoption) | Limited (OpenThread is C-based) | LOW |
| **Serial/RS-485** | Legacy sensors, industrial | Common baseline | Excellent (serialport crate) | HIGH |

### State of the Art

The OPC Foundation's UA Edge Translator converts Modbus, CIP, LoRaWAN, Matter and more into a unified OPC UA model using W3C Web of Things (WoT) Thing Descriptions for faster IIoT integration. Supported protocols include Modbus TCP, Rockwell CIP, Beckhoff ADS, LoRaWAN, Matter, and OCPP, with experimental support for Siemens S7Comm, Mitsubishi MC Protocol, BACnet, IEC 61850.

### Rust Implementation Status

**OPC-UA:**
- `async-opcua` (FreeOpcUa/async-opcua): Fully featured async client and server implementation, fork of locka99/opcua with broader goals
- `opcua` (locka99/opcua): OPC UA server/client API supporting embedded, micro, and nano profiles (537 GitHub stars)
- basysKom notes "great interest in implementing OPC UA servers in Rust among embedded customers" with EU Cyber Resilience Act driving adoption

**Modbus:**
- `modbus-core`: Pure no_std Rust library ideal for embedded (RTU + TCP)
- `tokio-modbus`: Async Modbus with RTU and TCP support
- `rmodbus`: Flexible framework for various transport mechanisms

**CAN Bus:**
- `zencan` (June 2025): New CANOpen protocol implementation
- `embedded-can`: HAL traits for CAN devices
- `socketcan`: Linux SocketCAN integration
- AUTOSAR has announced a Working Group for Rust in Automotive Software

**BLE:**
- `btleplug`: Cross-platform async BLE (Windows, macOS, Linux, iOS, Android)
- `esp32-nimble`: BLE for ESP32 with scanner, server, secure client support
- `bluetooth-hci`: no_std Bluetooth HCI implementation

**LoRaWAN:**
- `rust-lorawan`: no_std ecosystem with lora-phy, lorawan-encoding, lorawan-device
- `drogue-device`: Embassy-based IoT examples including LoRaWAN with OTA

### Recommendations

1. **Priority 1 (Industrial Foundation):**
   - Add OPC-UA client using `async-opcua` for SCADA integration
   - Add Modbus RTU/TCP using `tokio-modbus` for legacy devices
   - Add Serial/RS-485 support for direct sensor communication

2. **Priority 2 (Extended Connectivity):**
   - Add CAN bus support using `embedded-can` for automotive/industrial
   - Add BLE scanning/gateway using `btleplug` for sensor networks

3. **Priority 3 (Long-Range):**
   - Add LoRaWAN gateway support using `rust-lorawan`

---

## 2. Edge ML/AI

### Current State
- No ML inference capabilities

### Gap Analysis

| Capability | Use Case | Complexity | Rust Support | Priority |
|------------|----------|------------|--------------|----------|
| **ONNX Runtime** | General model inference | Medium | Via C bindings | HIGH |
| **TensorFlow Lite** | Quantized models | Medium | Via C bindings | MEDIUM |
| **Tract** | Pure Rust inference | Low | Native | HIGH |
| **Anomaly Detection** | Predictive maintenance | Medium | Custom/Tract | HIGH |
| **Time Series Forecasting** | Demand prediction | Medium | Custom | MEDIUM |

### State of the Art

The Edge AI chips market hit $7.08 billion in 2025, projected to reach nearly $57 billion by 2030. Key frameworks:

**Tract** - Pure Rust inference runtime:
- Supports ONNX and TFLite reading
- Small memory usage, predictable behavior
- Ideal for sensor gateways and SBCs
- Best for deterministic embedded systems

**TensorFlow Lite Micro:**
- Core runtime fits in 16 KB on ARM Cortex M3
- No OS, standard library, or dynamic allocation required
- Future focus on Generative AI at the Edge with new quantization for transformers

**ONNX Runtime:**
- Cross-framework interoperability (train in PyTorch, deploy anywhere)
- Significant inference speedups via hardware accelerators (Execution Providers)
- Crucial for real-time edge applications

**ArkFlow** (Rust):
- High-performance stream processing with AI integration
- Supports model loading, streaming inference, anomaly detection

### Research: Anomaly Detection at the Edge

Recent papers show that edge-deployed ML achieves:
- Shallow Neural Network: F1-score ~0.94
- Quantized TinyML: ~0.92 accuracy with efficiency tradeoff
- Decision Trees: 0.35ms latency, lowest memory for ultra-constrained devices

The Edge Computing based Anomaly Detection Algorithm (ECADA) handles both single-source and multi-source time series anomalies.

### Recommendations

1. **Priority 1 (Inference Foundation):**
   - Integrate `tract` for pure-Rust ONNX inference
   - Target: Anomaly detection on sensor streams
   - Model size constraints: <50MB for Pi compatibility

2. **Priority 2 (Statistical Methods):**
   - Implement lightweight anomaly detection (Z-score, IQR, isolation forest)
   - No external model dependency, pure Rust

3. **Priority 3 (Advanced):**
   - TFLite integration via FFI for specialized models
   - Hardware acceleration via Hailo/Coral when available

---

## 3. Real-time Processing

### Current State
- Batch ETL from Bronze to Silver
- No streaming/CEP capabilities

### Gap Analysis

| Capability | Use Case | Complexity | Rust Support | Priority |
|------------|----------|------------|--------------|----------|
| **Sliding Windows** | Rolling aggregations | Medium | ArkFlow, Arroyo | HIGH |
| **Tumbling Windows** | Time-based batches | Low | Multiple | HIGH |
| **Session Windows** | Activity-based grouping | Medium | Arroyo | MEDIUM |
| **Complex Event Processing** | Pattern detection | High | Limited | MEDIUM |
| **Watermarks** | Late data handling | Medium | Arroyo | MEDIUM |

### State of the Art

**Arroyo** (Rust):
- Outperforms Apache Flink by 5x or more
- Sliding, tumbling, and session windows with watermark processing
- 10x faster sliding windows than Flink
- Cloud-native with seamless scaling and recovery
- SQL-first for non-experts

**ArkFlow** (Rust):
- CNCF Cloud Native Landscape member
- Sliding window buffer with configurable window size, slide interval
- AI/ML integration for streaming inference

**Fluvio/InfinyOn** (Rust):
- WebAssembly for data processing packages
- Time and event-based windows
- Edge processing to reduce latency and bandwidth

### Implementation Path

```rust
// Example: Sliding window aggregation
struct SlidingWindow<T> {
    window_size: Duration,
    slide_interval: Duration,
    buffer: VecDeque<(Instant, T)>,
}

impl<T> SlidingWindow<T> {
    fn emit_if_ready(&mut self) -> Option<Vec<T>> {
        // Emit window contents at slide_interval
    }
}
```

### Recommendations

1. **Priority 1 (Basic Windowing):**
   - Implement tumbling windows for time-based aggregations
   - Implement sliding windows for rolling statistics
   - Integrate with existing ETL pipeline

2. **Priority 2 (Advanced):**
   - Add watermark support for late data
   - Implement session windows for activity grouping

3. **Priority 3 (CEP):**
   - Pattern matching on event streams
   - Consider embedding ArkFlow components

---

## 4. Edge-Cloud Sync

### Current State
- No cloud synchronization
- Data stays at edge

### Gap Analysis

| Capability | Use Case | Complexity | Rust Support | Priority |
|------------|----------|------------|--------------|----------|
| **Store-and-Forward** | Intermittent connectivity | Medium | Custom | HIGH |
| **Delta Sync** | Bandwidth efficiency | Medium | Custom | HIGH |
| **Conflict Resolution** | Multi-writer scenarios | High | rust-crdt | MEDIUM |
| **CRDTs** | Eventually consistent data | Medium | Good | MEDIUM |

### State of the Art

**Key Principles:**
- Local buffering (store-and-forward) with persistent queues
- Batching telemetry with exponential backoff and jitter
- CRDTs or operational transforms for latency-tolerant sync
- Distributed consensus (Paxos/Raft) for strong consistency needs

**CouchDB Replication:**
- Incremental, state-aware replication
- Tracks changes using revision trees, syncs only deltas
- Ideal for edge computing and hybrid architectures

**Couchbase Sync Gateway:**
- WebSocket-based protocol with enterprise features
- HA, automatic conflict resolution, delta sync
- Custom conflict resolvers

**SymmetricDS:**
- Open-source data synchronization
- Handles conflicts for concurrent updates

### Rust CRDT Libraries

**rust-crdt:**
- Well-tested, serializable CRDTs
- G-Counter, G-Set, LWW-Register, OR-Set, Merkle-Dag Register
- Deterministic local conflict resolution

**Automerge:**
- JSON CRDT by Martin Kleppmann
- Rust core with JS/WASM bindings
- Columnar encoding for efficiency

**Loro:**
- Rich text, list, map, movable tree support
- Based on Peritext and Fugue algorithms

### Sync Protocol Design

```
Edge Node                           Cloud
    |                                  |
    |---(1) Collect sensor data------->|
    |                                  |
    |---(2) Buffer locally (SQLite)--->|
    |                                  |
    |---(3) Batch & compress---------->|
    |                                  |
    |<--(4) ACK with checkpoint--------|
    |                                  |
    |---(5) Purge acknowledged-------->|
```

### Recommendations

1. **Priority 1 (Resilient Upload):**
   - Implement store-and-forward queue (SQLite-backed)
   - Delta compression for time-series data
   - Exponential backoff with jitter

2. **Priority 2 (Bi-directional):**
   - Configuration sync from cloud to edge
   - Use `rust-crdt` for conflict resolution

3. **Priority 3 (Advanced):**
   - Full CRDT-based document sync
   - Multi-edge coordination

---

## 5. Security

### Current State
- Basic authentication likely
- No hardware security integration

### Gap Analysis

| Capability | Use Case | Complexity | Rust Support | Priority |
|------------|----------|------------|--------------|----------|
| **TLS/mTLS** | Transport encryption | Low | rustls | HIGH |
| **Secure Boot** | Firmware integrity | High | Platform-specific | MEDIUM |
| **TPM Integration** | Key storage, attestation | High | tpm2-tss bindings | MEDIUM |
| **Data Encryption at Rest** | Privacy compliance | Medium | ring, aes-gcm | HIGH |
| **Zero Trust** | Access control | Medium | Custom | MEDIUM |

### State of the Art

**Secure Boot:**
- Ensures only authenticated, digitally signed code executes during startup
- Critical for mission-critical edge operations
- TPM market valued at $3.28 billion in 2025

**Hardware Security:**
- TPM provides secure key storage and device authentication
- HSM for hardware-based encryption
- NXP EdgeLock secure elements for IoT

**Attestation:**
- Static attestation verifies software integrity during power-up
- Runtime attestation for continuous verification
- Trust anchors at silicon level becoming standard

**Post-Quantum Cryptography:**
- Organizations preparing for quantum-safe algorithms
- EU Cyber Resilience Act driving security requirements

### Rust Security Ecosystem

- `rustls`: Modern TLS implementation
- `ring`: Cryptographic primitives (AES-GCM, ChaCha20-Poly1305)
- `webpki`: Certificate validation
- `tpm2-tss`: TPM 2.0 bindings (via FFI)
- `rcgen`: Certificate generation

### Recommendations

1. **Priority 1 (Transport Security):**
   - Implement mTLS for all external communication
   - Certificate-based authentication

2. **Priority 2 (Data Protection):**
   - Encrypt sensitive data at rest (AES-256-GCM)
   - Secure credential storage

3. **Priority 3 (Hardware Security):**
   - TPM integration for key storage
   - Secure boot verification

---

## 6. Visualization

### Current State
- Grafana dashboards (external)
- No embedded visualization

### Gap Analysis

| Capability | Use Case | Complexity | Rust Support | Priority |
|------------|----------|------------|--------------|----------|
| **Embedded Web UI** | Local monitoring | Medium | axum/actix-web | MEDIUM |
| **Grafana at Edge** | Full dashboard | Low | Deployment | HIGH |
| **Mobile Alerts** | Push notifications | Medium | External services | MEDIUM |
| **Real-time Charts** | Live data display | Medium | WebSocket + JS | MEDIUM |

### State of the Art

**Grafana at the Edge:**
- Can run directly on industrial IoT edge gateways
- Combined with InfluxDB (MING stack) for time-series
- Siemens embeds Grafana in Dashboard Designer for IIoT
- Reduces cloud dependency, provides immediate insights

**Embedding Options:**
- Snapshots: Moment-in-time interactive dashboard
- iframe embedding: Full Grafana panels in custom apps
- Public dashboards: Fully interactive for any time range

**AWS IoT SiteWise + Grafana:**
- Connect dashboards directly to edge software
- Access asset data stored on-premises

### Recommendations

1. **Priority 1 (Grafana Integration):**
   - Document Grafana deployment on Pi
   - Pre-built dashboards for NDP metrics
   - Direct TimescaleDB connection

2. **Priority 2 (Embedded UI):**
   - Lightweight status page (axum + htmx)
   - Real-time metric display
   - Configuration interface

3. **Priority 3 (Alerting):**
   - Push notification integration
   - Webhook-based alerts

---

## 7. Inter-node Communication

### Current State
- Single node operation
- No mesh/coordination

### Gap Analysis

| Capability | Use Case | Complexity | Rust Support | Priority |
|------------|----------|------------|--------------|----------|
| **Peer Discovery** | Auto-configuration | Medium | mDNS/DNS-SD | MEDIUM |
| **Data Sharing** | Distributed queries | High | Custom | LOW |
| **Leader Election** | Coordination | High | raft-rs | LOW |
| **Mesh Networking** | Resilient topology | High | Custom | LOW |

### State of the Art

**Edge Mesh Computing:**
- Distributes decision-making among edge devices
- Enables "self-healing" capability (reroute around failed nodes)
- ICN-EdgeMesh (2025): ML-based approach achieving 9.1-10 Mbps with ultra-low latency

**Peer-to-Peer Edge:**
- Swarm of intelligent connected devices sharing compute/data
- Novel mesh intelligence and coordinated functions
- All resources become accessible regardless of role (device, edge, cloud)

**Protocols:**
- Zigbee/Thread: Self-healing mesh networks
- Thread: IPv6-based (6LoWPAN), direct internet integration
- OpenThread: Open-source Thread implementation

### Rust Libraries

- `libp2p`: Peer-to-peer networking (IPFS foundation)
- `raft-rs`: Raft consensus implementation (TiKV)
- `mdns`: mDNS/DNS-SD for service discovery

### Recommendations

1. **Priority 1 (Discovery):**
   - mDNS-based peer discovery
   - Service announcement for NDP nodes

2. **Priority 2 (Data Exchange):**
   - Simple query forwarding between nodes
   - Aggregation at edge coordinator

3. **Priority 3 (Full Mesh):**
   - Distributed coordination
   - Fault-tolerant topology

---

## 8. Hardware Acceleration

### Current State
- CPU-only processing
- No accelerator support

### Gap Analysis

| Capability | Use Case | Complexity | Rust Support | Priority |
|------------|----------|------------|--------------|----------|
| **Hailo-8/8L** | Neural inference (Pi AI Kit) | Medium | Via C API | MEDIUM |
| **Coral TPU** | TensorFlow models | Medium | Via C API | MEDIUM |
| **GPU (Pi)** | Video processing | High | Limited | LOW |
| **FPGA** | Custom acceleration | Very High | Limited | LOW |

### State of the Art

**Raspberry Pi AI Kit:**
- Hailo-8L chip: 13 TOPS neural network inference
- Hailo-8 version: 26 TOPS
- M.2 2242 form factor with M.2 HAT+
- Best documentation and ease-of-use for beginners

**Performance Benchmarks:**
- NVIDIA Jetson Orin NX: 41.8 FPS (highest)
- Raspberry Pi 5 + Coral TPU: 21.5 FPS
- Power: Pi 5 + Coral at 8.3W, Jetson Nano at 7W

**NPU vs TPU:**
- NPUs often better for edge (low power, SoC integration)
- Rockchip RK3588: 6.0 TOPS NPU integrated

**Heterogeneous Architecture Trend:**
- 2025 sees shift toward mixed ASICs, FPGAs, NPUs
- Each optimized for different AI lifecycle stages

### Implementation Notes

Hardware acceleration requires:
1. Driver installation (platform-specific)
2. Model compilation for target accelerator
3. Runtime integration via C FFI

### Recommendations

1. **Priority 1 (Documentation):**
   - Document Hailo-8L setup with NDP
   - Model deployment guide

2. **Priority 2 (Integration):**
   - Optional Hailo runtime integration
   - Fallback to Tract for CPU-only

3. **Priority 3 (Advanced):**
   - Coral TPU support
   - Model optimization pipeline

---

## Priority Matrix

### High Impact, Low Complexity (Do First)
1. Modbus RTU/TCP support (tokio-modbus)
2. Serial/RS-485 ingestion
3. Store-and-forward sync queue
4. TLS/mTLS communication
5. Grafana deployment documentation

### High Impact, Medium Complexity (Plan Next)
1. OPC-UA client (async-opcua)
2. Tract ONNX inference
3. Sliding/tumbling windows
4. Data encryption at rest
5. Embedded status web UI

### Medium Impact, Medium Complexity (Roadmap)
1. BLE gateway (btleplug)
2. CAN bus support
3. Anomaly detection algorithms
4. CRDT-based sync
5. Peer discovery (mDNS)

### Lower Priority (Future)
1. LoRaWAN gateway
2. Full CEP engine
3. Hardware acceleration (Hailo/Coral)
4. Edge mesh networking
5. Zigbee/Thread integration

---

## Competitive Landscape

### Open Source Edge Platforms

| Platform | Strengths | Weaknesses | Fit for NDP |
|----------|-----------|------------|-------------|
| **KubeEdge** | K8s native, CNCF backed, ~70MB footprint | Requires K8s knowledge | Complement |
| **EdgeX Foundry** | Multi-protocol, Linux Foundation | Java/Go based, heavier | Learn from |
| **Azure IoT Edge** | Cloud integration, ML support | Vendor lock-in | Compete |
| **AWS Greengrass** | Lambda at edge | AWS dependency | Compete |

### NDP Differentiators to Develop

1. **Rust-native**: Memory safety without GC, smaller footprint
2. **Configuration-driven**: GitOps ETL without code deployment
3. **MCP Integration**: AI agent tooling (unique capability)
4. **Transparent DQ**: Layered data quality with full lineage
5. **Resource-efficient**: <2GB RAM target (vs typical >4GB)

---

## Implementation Roadmap

### Phase 1: Industrial Foundation (Q1-Q2)
- Modbus RTU/TCP source adapter
- OPC-UA client for SCADA
- Store-and-forward sync queue
- mTLS for all external communication

### Phase 2: Intelligence Layer (Q3)
- Tract ONNX inference integration
- Sliding/tumbling window aggregations
- Statistical anomaly detection
- Grafana dashboard templates

### Phase 3: Extended Connectivity (Q4)
- BLE sensor gateway
- CAN bus ingestion
- CRDT-based configuration sync
- Embedded status web UI

### Phase 4: Advanced Features (Future)
- LoRaWAN support
- Complex event processing
- Hardware acceleration (Hailo-8L)
- Edge mesh coordination

---

## Sources

### Protocol Support
- [OPC UA Edge Translator - OPC Foundation](https://github.com/OPCFoundation/UA-EdgeTranslator)
- [Industrial IoT Gateways Guide - Ubidots](https://ubidots.com/blog/top-industrial-iot-gateways/)
- [OPC UA and Rust in 2025 - basysKom](https://www.basyskom.de/en/opc-ua-and-rust-in-2025/)
- [async-opcua - GitHub](https://github.com/FreeOpcUa/async-opcua)
- [tokio-modbus - GitHub](https://github.com/slowtec/tokio-modbus)
- [modbus-core - GitHub](https://github.com/slowtec/modbus-core)
- [Zencan CANOpen - Jeff McBride](https://jeffmcbride.net/blog/2025/06/05/introducing-zencan/)
- [btleplug BLE - GitHub](https://github.com/deviceplug/btleplug)
- [rust-lorawan - GitHub](https://github.com/ivajloip/rust-lorawan)
- [LoRaWAN Applications in Rust - Tweede Golf](https://tweedegolf.nl/en/blog/69/lorawan-applications-in-rust)

### Edge ML/AI
- [7 Best Rust Frameworks for ML on Edge - Calmops](https://calmops.com/programming/rust/7-best-rust-ml-frameworks-edge-2025/)
- [Rust 2025 Memory Optimization for Embedded AI - Markaicode](https://markaicode.com/rust-2025-memory-optimization-embedded-ai-raspberry-pi-6/)
- [TinyML Frameworks - DFRobot](https://www.dfrobot.com/blog-13921.html)
- [Top 10 Edge AI Frameworks 2025 - Huebits](https://blog.huebits.in/top-10-edge-ai-frameworks-for-2025-best-tools-for-real-time-on-device-machine-learning/)
- [awesome-tinyml - GitHub](https://github.com/umitkacar/awesome-tinyml)
- [ArkFlow Stream Processing - GitHub](https://github.com/arkflow-rs/arkflow)

### Real-time Processing
- [Arroyo Stream Processing](https://www.arroyo.dev/)
- [10x Faster Sliding Windows - Arroyo Blog](https://www.arroyo.dev/blog/how-arroyo-beats-flink-at-sliding-windows/)
- [Fluvio Stateful DataFlow - Rust Forum](https://users.rust-lang.org/t/fluvio-stateful-dataflow-distributed-streaming-stream-processing-infrastructure/116732)
- [Event Stream Processing - Infinyon](https://www.infinyon.com/use-cases/event-stream-processing/)

### Edge-Cloud Sync
- [Edge-to-Cloud Sync That Never Flakes - Medium](https://medium.com/@Nexumo_/edge-to-cloud-sync-that-never-flakes-65b561117b5f)
- [CouchDB Replication for Edge - Medium](https://medium.com/@firmanbrilian/real-time-data-synchronization-across-edge-and-cloud-systems-using-couchdb-replication-f8bf97bd46c6)
- [Data Synchronization with SymmetricDS - freeCodeCamp](https://www.freecodecamp.org/news/data-synchronization-for-edge-computing/)
- [rust-crdt - GitHub](https://github.com/rust-crdt/rust-crdt)
- [CRDT Dictionary - Ian Duncan](https://www.iankduncan.com/engineering/2025-11-27-crdt-dictionary/)
- [Automerge CRDT](https://crdt.tech/implementations)

### Security
- [Edge Device Security Guide - Corvalent](https://corvalent.com/news/secure-boot-tpm-and-fips-readiness-checklist-for-edge-devices/)
- [Edge Computing Security 2025 - Otava](https://www.otava.com/blog/2025-trends-in-edge-computing-security/)
- [Azure IoT Edge Security - Microsoft](https://learn.microsoft.com/en-us/azure/iot-edge/security)
- [NXP EdgeLock IoT Security](https://www.nxp.com/applications/technologies/security/iot-security:EDGELOCK-IOT-SECURITY)

### Visualization
- [Industrial IoT Visualization with Grafana - Grafana Labs](https://grafana.com/blog/2025/01/27/industrial-iot-visualization-how-grafana-powers-industrial-automation-and-iiot/)
- [Grafana IoT Dashboard Guide - Robustel](https://www.robustel.store/blogs/industrial-iot-blog/grafana-iot-dashboard-guide)
- [Embedding Grafana Dashboards - Grafana Labs](https://grafana.com/blog/how-to-embed-grafana-dashboards-into-web-applications/)

### Inter-node Communication
- [Edge Mesh for Distributed Intelligence - Barbara](https://www.barbara.tech/blog/why-is-edge-mesh-the-next-hot-topic-for-distributed-intelligence)
- [Edge Computing with P2P - ACM](https://dl.acm.org/doi/10.1145/3313150.3313226)
- [IoT Networking Architecture 2025 - FloLive](https://flolive.net/blog/glossary/iot-networking-architecture-top-9-connectivity-methods-in-2025/)
- [OpenThread](https://openthread.io/)

### Hardware Acceleration
- [Top 10 Edge AI Hardware 2025 - Jaycon](https://www.jaycon.com/top-10-edge-ai-hardware-for-2025/)
- [Best Edge AI Boards Summer 2025 - Hackster](https://www.hackster.io/news/best-edge-ai-boards-summer-2025-edition-cfe8581d7460)
- [Raspberry Pi AI Kit](https://www.raspberrypi.com/products/ai-kit/)
- [Benchmarking Edge AI Platforms - Georgia Southern University](https://scholars.georgiasouthern.edu/en/publications/benchmarking-edge-ai-platforms-performance-analysis-of-nvidia-jet/)
- [NPU vs TPU for Edge - Gateworks](https://www.gateworks.com/choosing-the-right-ai-accelerator-npu-or-tpu-for-edge-and-cloud-applications/)

### Market & Platforms
- [Industrial Edge Market 2032 - MarketsandMarkets](https://www.marketsandmarkets.com/Market-Reports/industrial-edge-market-195348761.html)
- [Edge Computing Market 2026-2035 - GMInsights](https://www.gminsights.com/industry-analysis/edge-computing-market)
- [Top 10 Edge IoT Platforms - ZedIoT](https://zediot.com/blog/top-10-edge-iot-platforms-comparison-and-in-depth-analysis/)
- [KubeEdge](https://kubeedge.io/)
- [Open Source IoT Edge Projects - Medium](https://iskerrett.medium.com/open-source-iot-edge-projects-d9e79580c2d1)
