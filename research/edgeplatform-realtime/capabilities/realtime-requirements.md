# Real-Time Capability Architecture and Requirements

## Executive Summary

This document defines the technical requirements for transforming the Neural Data Platform (NDP) from a data collection and storage system into a **real-time edge inference and trigger platform** capable of executing ML inference and automated responses in **seconds or sub-second timeframes** on resource-constrained hardware (Raspberry Pi, <2GB RAM, Rust-based).

---

## 1. Latency Requirements by Application

### 1.1 Human Perception Thresholds

Understanding human perception is critical for determining acceptable latency in user-facing systems:

| Modality | Perception Threshold | Noticeable Delay | Sources |
|----------|---------------------|------------------|---------|
| Visual (instantaneous) | 13ms | 75-100ms | [PubNub Human Perception](https://www.pubnub.com/blog/how-fast-is-realtime-human-perception-and-technology/) |
| Audio temporal differences | 5ms | 30-40ms | [Measuring JND for Audio Latency](https://dl.acm.org/doi/fullHtml/10.1145/3678299.3678331) |
| Haptic/tactile feedback | 1-5ms | 50-85ms | [Haptic Feedback Latency](https://eureka.patsnap.com/article/haptic-feedback-latency-why-1ms-delay-is-required-for-realism) |
| Multi-modal (visual+haptic) | Variable | 70ms window | [Tactual Labs Research](https://www.tactuallabs.com/papers/howMuchFasterIsFastEnoughCHI15.pdf) |
| General reaction time | 100ms (instantaneous) | 250ms (average) | [Mental Chronometry](https://en.wikipedia.org/wiki/Mental_chronometry) |

**Key Insight**: Robert Miller's classic 1968 research established that **100ms is perceived as instantaneous** in human-computer interactions.

### 1.2 Control System Stability Requirements

Industrial and safety-critical systems have strict timing requirements:

| Application Domain | Required Latency | Jitter Tolerance | Source |
|-------------------|------------------|------------------|--------|
| Critical control loops | <50ms | <1ms | [Trout Software - Latency in ICS](https://www.trout.software/resources/tech-blog/latency-requirements-in-industrial-control-systems) |
| PLC scan cycles | 1-100ms | Deterministic | [PLC I/O Lag Time](https://koeed.com/blogs/%E6%96%B0%E9%97%BB/plc-input-output-lag-time-plc-system-response-time) |
| Safety-critical systems | Sub-millisecond | Zero tolerance | [Nuvation Low-Latency Networks](https://www.nuvation.com/resources/article/low-latency-networks-control-system-applications) |
| RTOS typical cycle time | <10ms | Equidistant | [Control Design RTOS](https://www.controldesign.com/home/article/11370594/industrial-machine-controls-real-time-operating-systems-are-critical-for-deterministic-control-control-design) |
| Emergency stop interrupts | <1ms | Immediate | [PLC Wikipedia](https://en.wikipedia.org/wiki/Programmable_logic_controller) |

**Real-world Impact**: In semiconductor manufacturing, 5ms lag can cause 0.5um wafer alignment errors. Unplanned stops cost automotive plants $22,000/minute.

### 1.3 NDP-Specific Application Requirements

Based on the current NDP use cases (air quality monitoring, weather data, environmental sensing):

| Application | Latency Target | Justification |
|------------|----------------|---------------|
| Air quality alert (health advisory) | <1 second | Human safety, time to respond |
| Weather-based trigger (irrigation) | <5 seconds | Process control, not safety-critical |
| Sensor anomaly detection | <100ms | Early warning, equipment protection |
| ML forecast refresh | <10 seconds | Near-real-time dashboards |
| Actuator control (HVAC) | <500ms | User comfort, not safety |
| Emergency shutdown trigger | <50ms | Safety-critical response |

### 1.4 Tiered Latency Model for NDP

**Tier 1 - Critical (Hard Real-Time)**: <10ms
- Emergency shutdowns
- Safety interlocks
- Requires: Dedicated hardware path, no OS jitter

**Tier 2 - Fast (Soft Real-Time)**: 10-100ms
- Anomaly detection and alerts
- Real-time sensor fusion
- Requires: Priority scheduling, bounded latency

**Tier 3 - Responsive (Near Real-Time)**: 100ms-1s
- ML inference triggers
- User-facing alerts
- Requires: Optimized async processing

**Tier 4 - Background (Best Effort)**: >1s
- Batch analytics
- Model retraining
- Dashboard updates

---

## 2. ML Model Constraints for Real-Time Edge Inference

### 2.1 Model Size vs Inference Speed Tradeoffs

| Model Type | Size | Typical Inference Time (Pi 4) | Use Case |
|-----------|------|------------------------------|----------|
| MobileNet-SSD | 27MB | 209ms | Object detection |
| YOLO11n (NCNN optimized) | ~6MB | 40-125ms | Real-time detection |
| TinyML models | <1MB | <10ms | Sensor classification |
| Quantized CNN | 2-10MB | 50-200ms | Edge classification |
| LLM (Llama 3.2:1b) | ~1GB | Load: 52ms (Rust), 1648ms (Python) | Text inference |

**Source**: [PyTorch Real-Time on Pi](https://docs.pytorch.org/tutorials/intermediate/realtime_rpi.html), [LLMPi Research](https://arxiv.org/html/2504.02118v1)

### 2.2 Quantization Effects on Latency

Quantization reduces model precision for faster inference:

| Precision | Memory Reduction | Speed Improvement | Accuracy Impact |
|-----------|------------------|-------------------|-----------------|
| FP32 -> FP16 | 2x | 2-4x throughput | Minimal |
| FP32 -> INT8 | 4x | 2-4x on CPU, 7x+ on GPU | 0-2% degradation |
| FP32 -> INT4 | 8x | Up to 7.73x vs FP16 | 1-5% degradation |
| FP8 | 4x vs FP16 | 2x vs BF16 | Near FP16 quality |

**Recommendation**: INT8 quantization is optimal for Raspberry Pi edge deployment, offering the best latency/accuracy tradeoff.

**Source**: [NVIDIA PTQ Blog](https://developer.nvidia.com/blog/optimizing-llms-for-performance-and-accuracy-with-post-training-quantization/), [Red Hat Quantization Study](https://developers.redhat.com/articles/2024/10/17/we-ran-over-half-million-evaluations-quantized-llms)

### 2.3 Streaming Inference Architectures

For continuous sensor data, streaming inference is essential:

**Apache Flink Approach**:
- Windowing and state management for real-time feature computation
- Joins streaming data with reference tables
- Sub-10ms latency for feature extraction

**Edge-Specific Patterns**:
- Micro-batching with configurable window sizes (10ms-1s)
- Sliding windows for continuous aggregation
- State checkpointing for fault tolerance

**Source**: [Confluent Flink ML Guide](https://www.confluent.io/blog/using-flink-for-model-inference-a-guide-for-realtime-ai-applications/)

### 2.4 Early-Exit and Anytime Prediction

Early-exit neural networks enable adaptive inference based on sample complexity:

| Feature | Benefit | Implementation |
|---------|---------|----------------|
| Multiple exit points | Stop when confidence threshold met | BranchyNet architecture |
| Adaptive computation | "Easy" samples exit early | Entropy-based thresholds |
| Resource optimization | 14x reduction in latency for distributed edge | Model partitioning |
| Anytime prediction | Valid predictions at any computational budget | AVCS confidence sequences |

**Key Insight**: Early-exit models solve "overthinking" where easy samples are processed deeper than necessary, wasting computation and potentially degrading accuracy.

**Source**: [ACM Early-Exit Survey](https://dl.acm.org/doi/full/10.1145/3698767), [Early-Exit Networks](https://www.emergentmind.com/topics/early-exit-networks)

### 2.5 Model Cascades (Fast Filter -> Slow Classifier)

Cascading uses efficient models for easy samples, reserving complex models for hard cases:

```
Input -> [Fast Filter] -> 95% exit with high confidence
              |
              v (5% uncertain)
        [Slow Classifier] -> Final prediction
```

**Performance Gains**:
- 2-3x cost savings across latency-accuracy spectrum
- 14x reduction in communication costs (edge-to-cloud)
- Graceful degradation under load

**Implementation**: CascadeServe, Cascadia frameworks

**Source**: [CascadeServe](https://arxiv.org/abs/2406.14424), [Cascadia LLM Cascades](https://arxiv.org/html/2506.04203)

---

## 3. Trigger and Actuation Architecture

### 3.1 GPIO Timing Precision on Raspberry Pi

| Method | Timing Precision | Jitter | Use Case |
|--------|------------------|--------|----------|
| RPi.GPIO (Python) | ~100us | +/-100us | Non-critical |
| pigpio (DMA-based) | 5us sampling | +/-10us | Accurate timing |
| Hardware PWM | 1us | <1us | Precise signals |
| Direct GPIO (C, interrupts disabled) | 0.1us | 0.1us | Ultra-precise |
| Kernel interrupt | 4ms worst case | Variable | Avoid for real-time |

**Critical Issue**: Linux interrupt latency can spike to 4ms. Real-time applications require either:
1. Dedicated PWM hardware
2. DMA-based libraries (pigpio)
3. External microcontroller for timing-critical signals

**Source**: [Raspberry Pi Forums GPIO Timing](https://forums.raspberrypi.com/viewtopic.php?t=168696), [Hardware PWM Guide](https://nerdhut.de/2016/05/09/exact-timings-raspberry-pi/)

### 3.2 MQTT vs Direct Actuator Control

| Method | Typical Latency | QoS Tradeoffs | Best For |
|--------|-----------------|---------------|----------|
| MQTT QoS 0 | 1-10ms | No guarantee, fastest | Non-critical telemetry |
| MQTT QoS 1 | 5-50ms | At-least-once, some overhead | Most IoT scenarios |
| MQTT QoS 2 | 10-100ms | Exactly-once, highest overhead | Financial/critical |
| Direct GPIO | <1ms | No network, deterministic | Local actuators |
| Local pub/sub | 0.1-1ms | No network hop | Edge-to-edge |

**Recommendation**: For sub-10ms triggers, use direct GPIO or local pub/sub. Reserve MQTT for non-critical remote communication.

**Source**: [MQTT Protocol Guide](https://www.integrasources.com/blog/mqtt-protocol-iot-devices/), [EMQ Performance Testing](https://www.emqx.com/en/blog/a-beginner-guide-to-mqtt-performance-testing)

### 3.3 Deterministic Scheduling in Rust

Rust patterns for deterministic real-time behavior:

| Pattern | Crate | Use Case |
|---------|-------|----------|
| RTIC (interrupt-driven) | `rtic` | Task prioritization, preemption |
| Embassy (async embedded) | `embassy-*` | no_std async, efficient I/O |
| Priority scheduling | `tokio` with custom scheduler | Soft real-time |
| Static allocation | `heapless` | Avoid allocation jitter |
| Lock-free queues | `crossbeam` | Inter-task communication |

**Source**: [Ferrous Systems Embedded Concurrency](https://ferrous-systems.com/blog/embedded-concurrency-patterns/), [Embedded Rust Ecosystem](https://www.systemscape.com/blog/2024/state-of-embedded-rust-2/)

### 3.4 Real-Time OS Considerations

For hard real-time requirements on Pi:

| Option | Latency Guarantee | Complexity | Recommendation |
|--------|-------------------|------------|----------------|
| Linux PREEMPT_RT | ~100us worst case | Medium | Soft real-time |
| Xenomai/RTAI | ~10us worst case | High | Hard real-time |
| Zephyr RTOS | Deterministic | High | Dedicated MCU |
| Bare metal Rust | Sub-microsecond | Very high | Critical paths only |

**NDP Recommendation**: Linux PREEMPT_RT kernel for most use cases; offload sub-10ms requirements to dedicated MCU (RP2040, ESP32).

---

## 4. Stream Processing for Real-Time

### 4.1 Event-Time vs Processing-Time

| Approach | Description | Latency | Use Case |
|----------|-------------|---------|----------|
| Processing-time | Use system clock when event processed | Lowest | Real-time triggers |
| Event-time | Use timestamp embedded in event | Higher (buffering) | Accurate analytics |
| Ingestion-time | Use timestamp when event ingested | Medium | Compromise |

**NDP Current State**: Uses event-time (sensor timestamps), appropriate for analytics but adds latency for triggers.

**Recommendation**: Implement dual-path processing:
- Fast path: Processing-time for triggers
- Analytics path: Event-time for accuracy

### 4.2 Micro-Batching vs True Streaming

| Approach | Latency | Throughput | Memory | Best For |
|----------|---------|------------|--------|----------|
| True streaming | <10ms | Lower | Low | Real-time triggers |
| Micro-batching (10ms) | 10-50ms | Medium | Medium | Low-latency analytics |
| Micro-batching (1s) | 1-5s | High | Higher | Batch-like processing |

**Current NDP Architecture**: Async channels with `mpsc::channel(1000)` capacity suggest batch-oriented design.

**Gap**: No micro-batching or true streaming mode for real-time triggers.

### 4.3 Windowing Strategies for Real-Time

| Window Type | Description | Latency Impact | Use Case |
|-------------|-------------|----------------|----------|
| Tumbling | Fixed, non-overlapping | Emit at window end | Aggregations |
| Sliding | Overlapping windows | Continuous emission | Smoothing |
| Session | Gap-based | Variable | User sessions |
| Global | All events | Highest | Batch |

**Recommendation for NDP**:
- Sliding windows (1-5 min) for air quality moving averages
- Tumbling windows (1s) for anomaly detection
- Session windows for sensor connection monitoring

### 4.4 State Management for Streaming ML

| State Type | Storage | Recovery Time | Use Case |
|-----------|---------|---------------|----------|
| In-memory (volatile) | RAM | Lost on restart | Transient aggregations |
| RocksDB (embedded) | Disk | Fast recovery | Checkpointed state |
| TimescaleDB (Silver) | External DB | Query-time | Historical features |
| etcd (config) | Distributed | Fast | Configuration state |

**NDP Current State**: Uses etcd for config, TimescaleDB for Silver layer, Parquet for Bronze.

**Gap**: No in-memory state management for streaming ML features.

---

## 5. Hardware Acceleration Options

### 5.1 Hailo-8L Performance Characteristics

| Metric | Value | Source |
|--------|-------|--------|
| Compute power | 13 TOPS | [Seeed Studio Comparison](https://www.seeedstudio.com/blog/2024/07/16/raspberry-pi-ai-kit-vs-coral-usb-accelerator-vs-coral-m-2-accelerator-with-dual-edge-tpu/) |
| YOLOv6n inference | 9-13ms | [Frigate Discussions](https://github.com/blakeblackshear/frigate/discussions/21587) |
| YOLOv9t inference | 18ms | [SBC Nexus Comparison](https://sbcnexus.com/articles/71202049/coral-tpu-vs-hailo-the-ultimate-showdown-for-edge-ai-dominance/) |
| Power consumption | ~2.5W | [Buy Zero Comparison](https://buyzero.de/en/blogs/news/edge-ai-showdown-hailo-vs-coral-which-chip-is-right-for-you) |
| Interface | PCIe (M.2) | Native Pi 5 support |

**Advantages**: Higher TOPS, complex model support, better sustained throughput.
**Disadvantages**: More expensive, higher power, limited model ecosystem.

### 5.2 Coral TPU Inference Times

| Metric | Value | Source |
|--------|-------|--------|
| Compute power | 4 TOPS (single), 8 TOPS (dual) | [Coral Benchmarks](https://www.coral.ai/docs/edgetpu/benchmarks/) |
| Typical inference | ~10ms | [Frigate Hardware Guide](https://docs.frigate.video/frigate/hardware/) |
| Power consumption | 2W per TOPS | Official specs |
| Interface | USB 3.0, M.2 | Multiple form factors |

**Advantages**: Lower power, mature ecosystem, TensorFlow Lite support.
**Disadvantages**: Limited TOPS, poor night vision (model-dependent).

### 5.3 FPGA for Ultra-Low Latency

| Metric | Value | Source |
|--------|-------|--------|
| Typical latency | 5-60us | [Rexys FPGA Latency](https://rexys.io/low-latency-the-hidden-superpower-of-fpgas/) |
| Deterministic timing | Yes (clock-cycle level) | [Fidus FPGA AI](https://fidus.com/blog/custom-fpga-solutions-for-ai-acceleration-in-embedded-applications/) |
| Power consumption | 10W typical | [Promwad Edge AI](https://promwad.com/news/edge-ai-live-production-fpga-asic-transcoding-analytics) |
| Development complexity | Very high | Custom HDL required |

**Use Case**: When sub-100us deterministic latency is required. Cost and complexity typically prohibitive for NDP scale.

### 5.4 Neuromorphic Chips (Akida, Loihi)

| Chip | Performance | Power | Key Feature | Source |
|------|-------------|-------|-------------|--------|
| Akida Pulsar | 100x latency reduction vs conventional | <1mW (Pico) | Event-based processing | [BrainChip Akida](https://medium.com/@theagipodcast/brainchips-akida-neuromorphic-processor-bringing-ai-to-the-edge-0aed37968a02) |
| Loihi 2 | 75x lower latency vs Jetson | 1000x more efficient | Spiking neural networks | [Intel Loihi Principles](https://arxiv.org/html/2503.18002v2) |
| Intel NorthPole | Brain-inspired | 1/1000th GPU power | Emerging | [Neuromorphic Robotics](https://robocloud-dashboard.vercel.app/learn/blog/neuromorphic-robotics-2026) |

**Key Metrics**:
- Akida: 1.5ms latency, 650K+ predictions/second
- Event cameras: 1us temporal resolution vs 33ms for traditional

**Relevance to NDP**: Neuromorphic chips excel at event-driven sensor data - natural fit for air quality spikes and anomaly detection.

### 5.5 Hardware Acceleration Recommendation Matrix

| NDP Use Case | Recommended Accelerator | Expected Latency |
|--------------|------------------------|------------------|
| Object detection | Hailo-8L | 10-20ms |
| Classification | Coral TPU | ~10ms |
| Anomaly detection (SNN) | Akida | <2ms |
| Ultra-low latency triggers | FPGA or MCU | <100us |
| General ML inference | Pi 5 CPU (quantized) | 50-200ms |

---

## 6. Communication Latency

### 6.1 MQTT QoS vs Latency Tradeoffs

| QoS Level | Delivery Guarantee | Typical Latency | Resource Usage |
|-----------|-------------------|-----------------|----------------|
| QoS 0 | At most once | 1-5ms | Lowest |
| QoS 1 | At least once | 5-20ms | Medium |
| QoS 2 | Exactly once | 10-50ms | Highest |

**Finding**: "Latency is not much influenced by the QoS level as is by the message size" - larger messages dominate latency.

**Source**: [PMC MQTT Latency Study](https://pmc.ncbi.nlm.nih.gov/articles/PMC11935751/)

### 6.2 Local Pub/Sub Alternatives

| System | Latency | Throughput | Use Case |
|--------|---------|------------|----------|
| NanoMQ | <1ms local | 1M msgs/sec | Local edge broker |
| ZeroMQ | <100us | Very high | In-process |
| Tokio channels | <1us | Millions/sec | Same process |
| Unix sockets | 10-100us | High | Local IPC |

**Source**: [NanoMQ](https://mqtt.org/software/)

### 6.3 Shared Memory IPC

For maximum performance between processes on the same device:

| Technique | Latency | Throughput | Rust Crate |
|-----------|---------|------------|------------|
| Shared memory ringbuffer | <100ns | GB/s | `shmem-ipc` |
| Memory-mapped files | ~1us | GB/s | `memmap2` |
| Lock-free SPSC queues | <50ns | Very high | `ringbuf` |

**Key Insight**: "For cases where latency really matters, the maintenance overhead of using shared memory is worth it."

**Source**: [shmem-ipc GitHub](https://github.com/diwic/shmem-ipc), [IPC Ping Pong Comparison](https://3tilley.github.io/posts/simple-ipc-ping-pong/)

### 6.4 DMA for Sensor Data

Direct Memory Access enables zero-copy data transfer:

| Benefit | Description |
|---------|-------------|
| Zero CPU overhead | DMA controller handles transfer |
| Zero copy | Data stays in place |
| Lower latency | Parallel to CPU execution |
| Cache efficiency | Data remains in original location |

**Rust Implementation**: `embedded-dma` HAL traits, careful memory ordering with `Ordering::Release` and `Ordering::Acquire`.

**Source**: [Embedonomicon DMA](https://docs.rust-embedded.org/embedonomicon/dma.html), [Safe DMA Blog](https://blog.japaric.io/safe-dma/)

---

## 7. Rust Real-Time Capabilities

### 7.1 no_std for Determinism

Running without the standard library eliminates OS-dependent jitter:

| Feature | std | no_std |
|---------|-----|--------|
| Heap allocation | Yes (allocator) | No (static only) |
| Threading | OS threads | Interrupts/async |
| I/O | OS syscalls | Direct hardware |
| Timing | System clock | Hardware timers |
| Determinism | Variable | High |

**Crates for no_std**:
- `heapless`: Static data structures
- `embedded-hal`: Hardware abstraction
- `cortex-m`: ARM Cortex-M support
- `embassy`: Async runtime for embedded

**Source**: [Nine Rules for Embedded Rust](https://medium.com/data-science/nine-rules-for-running-rust-on-embedded-systems-b0c247ee877e)

### 7.2 Async vs Synchronous for Latency

| Approach | Latency | CPU Efficiency | Complexity |
|----------|---------|----------------|------------|
| Synchronous blocking | Highest | Low | Simple |
| Async (tokio) | Medium | High | Medium |
| Interrupt-driven (RTIC) | Lowest | Highest | Higher |
| Embassy async embedded | Low | High | Medium |

**Key Finding**: "In async/await patterns, tasks are usually executed at the same priority, meaning a single task can use a disproportionate amount of CPU time before yielding."

**Recommendation**: Use RTIC or Embassy for hard real-time; tokio with careful priority management for soft real-time.

### 7.3 Memory Allocation Strategies

| Strategy | Latency Impact | Use Case |
|----------|----------------|----------|
| Static allocation | None | Fixed-size buffers |
| Arena/bump allocator | Minimal | Temporary allocations |
| Object pools | Minimal | Reusable objects |
| General heap (jemalloc) | Variable | Non-real-time paths |

**Crates**:
- `heapless`: `Vec`, `String`, `HashMap` with fixed capacity
- `bumpalo`: Arena allocation
- `object-pool`: Pre-allocated object pools

### 7.4 Priority Scheduling

| Pattern | Implementation | Use Case |
|---------|----------------|----------|
| RTIC priority levels | Hardware interrupt priorities | Embedded |
| tokio spawn_blocking | Move CPU work off async runtime | General |
| Custom executor | Priority-aware task scheduling | Advanced |
| nice/renice | OS process priority | Linux |

### 7.5 Watchdog and Timeout Patterns

Preventing system hangs in real-time contexts:

| Pattern | Crate | Description |
|---------|-------|-------------|
| Hardware watchdog | `rp2040-hal::watchdog` | Reset on hang |
| Task watchdog | `task-watchdog` | Multi-task monitoring |
| Timeout wrapping | `tokio::time::timeout` | Bounded operations |
| Heartbeat monitoring | Custom | Liveness detection |

**Priority Inversion Warning**: Classic bug where high-priority task waits on low-priority task holding a mutex. Solution: Priority inheritance or lock-free data structures.

**Source**: [Watchdog Best Practices](https://interrupt.memfault.com/blog/firmware-watchdog-best-practices), [Priority Inversion Wikipedia](https://en.wikipedia.org/wiki/Priority_inversion)

---

## 8. Testing and Validation

### 8.1 Latency Measurement and Profiling

| Tool | Type | Use Case |
|------|------|----------|
| Criterion | Statistical benchmarks | Function latency |
| Iai | Instruction counting | Cache/memory analysis |
| flamegraph | Visual profiling | Hotspot identification |
| perf | Linux profiler | System-wide analysis |
| pprof-rs | CPU profiler | Rust-native profiling |

**Criterion Example**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn inference_benchmark(c: &mut Criterion) {
    c.bench_function("ml_inference", |b| b.iter(|| {
        model.predict(black_box(&input))
    }));
}
```

**Source**: [Criterion Rust](https://github.com/bheisler/criterion.rs), [Rust Performance Book](https://nnethercote.github.io/perf-book/benchmarking.html)

### 8.2 Jitter Analysis

Measuring timing variability:

| Metric | Description | Target |
|--------|-------------|--------|
| Mean latency | Average response time | Low |
| P99 latency | 99th percentile | Bounded |
| Max latency | Worst case | Critical |
| Jitter | Standard deviation | Minimal |

**Tools**:
- `rust-benchmark`: p95/p99 tracking
- Custom histograms with `hdrhistogram`
- GPIO toggle + oscilloscope for hardware timing

### 8.3 Worst-Case Execution Time (WCET) Analysis

| Approach | Accuracy | Effort | Tool |
|----------|----------|--------|------|
| Manual counting | High | Very high | None |
| Static analysis | Conservative | Medium | aiT, RapiTime |
| Measurement-based | Optimistic | Low | End-to-end tests |
| Hybrid | Balanced | Medium | Combined |

**Challenges**:
- Cache effects cause variability
- Multi-core shared resources
- Dynamic runtime behavior

**Source**: [LDRA WCET](https://ldra.com/capabilities/wcet/), [WCET Wikipedia](https://en.wikipedia.org/wiki/Worst-case_execution_time)

### 8.4 Real-Time Testing Frameworks

| Framework | Focus | Language |
|-----------|-------|----------|
| Loom | Concurrency testing | Rust |
| proptest | Property-based | Rust |
| QEMU | Hardware emulation | C/Rust |
| Renode | Multi-device simulation | C# |

### 8.5 CI/CD for Latency Regression

**Best Practices**:
1. Run Criterion benchmarks on every PR
2. Set regression thresholds (e.g., 10% degradation = failure)
3. Use dedicated benchmark hardware (avoid cloud variability)
4. Track p99 latency over time
5. Alert on latency regressions before merge

**Tools**:
- Bencher: Continuous benchmarking SaaS
- GitHub Actions with dedicated runners
- Custom dashboards (Grafana + InfluxDB)

**Source**: [Bencher](https://bencher.dev/learn/benchmarking/rust/criterion/)

---

## 9. Gap Analysis: Current NDP vs Real-Time Requirements

### 9.1 Current NDP Architecture Assessment

Based on code review of `core/src/`:

| Component | Current State | Real-Time Gap |
|-----------|---------------|---------------|
| Ingestion Coordinator | Async tokio, `mpsc::channel(1000)` | No priority scheduling |
| Storage (Parquet) | Batch-oriented writes | Not streaming |
| Storage (TimescaleDB) | Query-time analytics | Not real-time triggers |
| ML (FANN Adapter) | Mock model, async predict | No quantization, no accelerator |
| Communication | MQTT for ingestion | No local pub/sub fast path |
| Triggers | None implemented | Major gap |

### 9.2 Specific Gaps Identified

**1. No Real-Time Trigger Path**
- Current: Data flows to storage only
- Needed: Parallel path for immediate triggers

**2. No Hardware Acceleration**
- Current: CPU-only inference (mock)
- Needed: Hailo/Coral/Akida integration

**3. No Streaming State Management**
- Current: Stateless parsing/routing
- Needed: Windowed aggregations, feature state

**4. No Priority Scheduling**
- Current: Single-priority async
- Needed: Tiered latency paths

**5. No Latency Monitoring**
- Current: Record counts only (`CoordinatorStats`)
- Needed: p99 latency tracking, WCET analysis

**6. No Zero-Copy Paths**
- Current: `record.point.clone()` in routing
- Needed: Zero-copy for hot paths

### 9.3 Architectural Changes Required

```
Current Architecture:
Sources -> Channel -> Router -> Storage (Parquet/TimescaleDB)
                        |
                        v
                    [No triggers]

Required Architecture:
Sources -> Channel -> Classifier
                        |
            +-----------+-----------+
            |           |           |
            v           v           v
        Fast Path   Normal Path  Batch Path
        (<10ms)     (<1s)        (>1s)
            |           |           |
            v           v           v
        Triggers    Storage     Analytics
        (GPIO,      (Parquet,   (ML Training,
        Local PubSub) TimescaleDB) Aggregations)
```

---

## 10. Recommendations for Real-Time NDP

### 10.1 Immediate Actions (Phase 1)

1. **Add latency instrumentation**
   - Wrap critical paths with timing
   - Track p50, p95, p99 latencies
   - Integrate with Grafana dashboards

2. **Implement dual-path routing**
   - Fast path: Bypass storage for triggers
   - Normal path: Current storage flow

3. **Add local pub/sub**
   - `tokio::sync::broadcast` for in-process
   - Unix sockets or ZeroMQ for cross-process

### 10.2 Short-Term Improvements (Phase 2)

1. **Streaming windowing**
   - Implement sliding windows for aggregations
   - State management for ML features

2. **Model quantization**
   - INT8 quantization for inference models
   - ONNX Runtime integration

3. **Hardware accelerator support**
   - Abstract accelerator interface trait
   - Hailo-8L implementation (Pi 5)
   - Coral TPU fallback

### 10.3 Long-Term Architecture (Phase 3)

1. **RTIC/Embassy for critical paths**
   - Offload <10ms triggers to dedicated task
   - Interrupt-driven for emergency shutdowns

2. **Neuromorphic integration**
   - Akida for anomaly detection
   - Event-driven sensor processing

3. **Zero-copy throughout**
   - DMA for sensor ingestion
   - Shared memory for IPC
   - Avoid cloning in hot paths

### 10.4 Rust Crate Recommendations

| Category | Crate | Purpose |
|----------|-------|---------|
| Async runtime | `tokio` | General async |
| Embedded async | `embassy` | no_std async |
| Real-time | `rtic` | Priority scheduling |
| Benchmarking | `criterion` | Latency measurement |
| Static allocation | `heapless` | Fixed-size structures |
| Zero-copy | `bytes`, `zerocopy` | Buffer management |
| IPC | `shmem-ipc` | Shared memory |
| ML inference | `ort` (ONNX Runtime) | Optimized inference |
| Watchdog | `task-watchdog` | Multi-task monitoring |

---

## 11. Summary: What NDP Needs to Become Real-Time

### 11.1 Hardware Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | Pi 4 (4GB) | Pi 5 (8GB) |
| ML Accelerator | Coral USB | Hailo-8L M.2 |
| Storage | microSD | NVMe SSD |
| RAM | 2GB | 4-8GB |
| OS | Linux PREEMPT_RT | + Zephyr for MCU |

### 11.2 Latency Budget

| Stage | Target | Current | Gap |
|-------|--------|---------|-----|
| Sensor ingestion | 1ms | ~10ms | Medium |
| Parsing/routing | 0.5ms | ~5ms | Medium |
| ML inference | 10ms | N/A (mock) | Major |
| Trigger evaluation | 0.5ms | N/A | Major |
| Actuator command | 1ms | N/A | Major |
| **Total (fast path)** | **<15ms** | **N/A** | **Major** |

### 11.3 Key Metrics to Track

1. **End-to-end latency**: Sensor event to trigger action
2. **P99 latency**: Worst-case bounded performance
3. **Jitter**: Timing variability
4. **Throughput**: Events per second under load
5. **Accuracy vs latency tradeoff**: Model performance at different latencies

### 11.4 Success Criteria

| Requirement | Metric | Target |
|-------------|--------|--------|
| Fast path latency | P99 | <50ms |
| Critical trigger latency | P99 | <10ms |
| ML inference | Average | <20ms |
| Jitter | Standard deviation | <5ms |
| Throughput | Events/second | >1000 |
| Memory footprint | Peak RAM | <512MB |
| Power consumption | Steady state | <10W |

---

## Sources

### Human Perception and Latency
- [PubNub - Human Perception and Technology](https://www.pubnub.com/blog/how-fast-is-realtime-human-perception-and-technology/)
- [Haptic Feedback Latency Research](https://eureka.patsnap.com/article/haptic-feedback-latency-why-1ms-delay-is-required-for-realism)
- [ACM - Measuring JND for Audio Latency](https://dl.acm.org/doi/fullHtml/10.1145/3678299.3678331)

### Edge ML and Inference
- [PyTorch - Real-Time Inference on Raspberry Pi](https://docs.pytorch.org/tutorials/intermediate/realtime_rpi.html)
- [LearnOpenCV - YOLO11 on Raspberry Pi](https://learnopencv.com/yolo11-on-raspberry-pi/)
- [arXiv - LLMPi: Optimizing LLMs on Raspberry Pi](https://arxiv.org/html/2504.02118v1)
- [Roboflow - Inference Latency](https://blog.roboflow.com/inference-latency/)

### Quantization and Optimization
- [NVIDIA - Post-Training Quantization](https://developer.nvidia.com/blog/optimizing-llms-for-performance-and-accuracy-with-post-training-quantization/)
- [Red Hat - Quantized LLM Evaluations](https://developers.redhat.com/articles/2024/10/17/we-ran-over-half-million-evaluations-quantized-llms)
- [NVIDIA - Mastering LLM Inference Optimization](https://developer.nvidia.com/blog/mastering-llm-techniques-inference-optimization/)

### Hardware Accelerators
- [Seeed Studio - Pi AI Kit vs Coral Comparison](https://www.seeedstudio.com/blog/2024/07/16/raspberry-pi-ai-kit-vs-coral-usb-accelerator-vs-coral-m-2-accelerator-with-dual-edge-tpu/)
- [SBC Nexus - Coral vs Hailo](https://sbcnexus.com/articles/71202049/coral-tpu-vs-hailo-the-ultimate-showdown-for-edge-ai-dominance/)
- [Coral - Edge TPU Benchmarks](https://www.coral.ai/docs/edgetpu/benchmarks/)
- [Open Neuromorphic - Akida](https://open-neuromorphic.org/neuromorphic-computing/hardware/akida-brainchip/)

### Real-Time and Embedded Rust
- [Ferrous Systems - Embedded Concurrency Patterns](https://ferrous-systems.com/blog/embedded-concurrency-patterns/)
- [Systemscape - State of Embedded Rust](https://www.systemscape.com/blog/2024/state-of-embedded-rust-2/)
- [awesome-embedded-rust GitHub](https://github.com/rust-embedded/awesome-embedded-rust)
- [Embedonomicon - DMA](https://docs.rust-embedded.org/embedonomicon/dma.html)

### Industrial Control and WCET
- [Trout Software - Latency in Industrial Control Systems](https://www.trout.software/resources/tech-blog/latency-requirements-in-industrial-control-systems)
- [LDRA - WCET Analysis](https://ldra.com/capabilities/wcet/)
- [Control Design - RTOS for Deterministic Control](https://www.controldesign.com/home/article/11370594/industrial-machine-controls-real-time-operating-systems-are-critical-for-deterministic-control-control-design)

### Communication and IPC
- [shmem-ipc GitHub](https://github.com/diwic/shmem-ipc)
- [3tilley - IPC Ping Pong Comparison](https://3tilley.github.io/posts/simple-ipc-ping-pong/)
- [EMQ - MQTT Performance Testing](https://www.emqx.com/en/blog/a-beginner-guide-to-mqtt-performance-testing)

### Benchmarking and Testing
- [Criterion.rs GitHub](https://github.com/bheisler/criterion.rs)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/benchmarking.html)
- [Bencher - Continuous Benchmarking](https://bencher.dev/learn/benchmarking/rust/criterion/)

### Early-Exit and Model Cascades
- [ACM - Early-Exit Neural Networks Survey](https://dl.acm.org/doi/full/10.1145/3698767)
- [arXiv - CascadeServe](https://arxiv.org/abs/2406.14424)
- [arXiv - Cascadia LLM Serving](https://arxiv.org/html/2506.04203)

### FPGA and Neuromorphic
- [Rexys - FPGA Low Latency](https://rexys.io/low-latency-the-hidden-superpower-of-fpgas/)
- [arXiv - Neuromorphic Principles for LLMs on Loihi 2](https://arxiv.org/html/2503.18002v2)
- [BrainChip Akida - Medium](https://medium.com/@theagipodcast/brainchips-akida-neuromorphic-processor-bringing-ai-to-the-edge-0aed37968a02)

---

*Document generated: 2026-01-18*
*Research conducted for: Neural Data Platform Edge Real-Time Capabilities*
