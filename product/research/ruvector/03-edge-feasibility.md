# RuVector Edge Deployment Feasibility on Raspberry Pi 5

**Research Date**: 2026-02-10
**Revised**: 2026-02-10 (actual Pi 5 specs: 16GB RAM, 1TB NVMe SSD)
**Platform**: Raspberry Pi 5 (16GB RAM, 1TB NVMe SSD, Cortex-A76 quad-core @ 2.4GHz)
**Target**: Neural Data Platform (NDP) v1.1.21+

> **MAJOR REVISION:** This document was originally written assuming a 4GB Pi 5 with microSD. Actual production hardware is **16GB RAM + 1TB NVMe SSD**. This fundamentally changes feasibility verdicts — every ruvector deployment option is now viable. The document has been revised with updated hardware profile, verdicts, and recommendations.

---

## 1. Current NDP Resource Budget on Pi 5

Measured from production `docker stats` output (2026-02-10):

| Service              | Memory Used | Memory Limit  | CPU   | Notes                        |
|----------------------|------------|---------------|-------|------------------------------|
| air-quality-app      | 123 MB     | 512 MB        | 0.09% | Main ingestion binary        |
| TimescaleDB          | 308 MB     | *uncapped*    | 0.01% | Silver layer                 |
| Grafana              | 98 MB      | *uncapped*    | 0.22% | Dashboards                   |
| etcd                 | 89 MB      | *uncapped*    | 0.30% | Config store                 |
| ndp-mcp-server       | 19 MB      | 96 MB         | 3.54% | MCP interface                |
| mqtt-broker          | 8 MB       | *uncapped*    | 0.03% | Mosquitto                    |
| **Total used**       | **~646 MB**|               | **~4.2%** |                          |
| **Total RAM**        | **16,384 MB** |            |       | Pi 5 16GB model              |
| **Available**        | **~15,354 MB** |           | **~96%** |                          |

**Storage:** 1TB NVMe SSD (~100K random IOPS, mmap-viable for HNSW indices)

### What This Changes from Original Analysis

The original analysis assumed 4GB Pi with ~600-1,000MB for ruvector. At 16GB with 15.3GB available, **every ruvector deployment option is viable.** The constraint shifts from memory to complexity management.

| For RuVector            | Original (4GB) | Actual (16GB) | Change     |
|------------------------|----------------|---------------|------------|
| Available RAM           | 600-1,000 MB   | ~15,300 MB    | **15x more** |
| Storage type            | microSD assumed | NVMe SSD      | **25x faster random IOPS** |
| Full container viable?  | No             | **Yes (~1GB)** | Unlocked   |
| GNN module viable?      | No             | **Yes (~320MB)** | Unlocked |
| f32 (no compression)?   | Need PQ8       | **Fine for years** | Simplified |

---

## 2. NDP Data Volume Projections

Based on stream configurations:

| Stream              | Interval  | Points/Day | Points/Month | Points/Year |
|---------------------|-----------|-----------|-------------|------------|
| air-quality (MQTT)  | ~1 min    | 1,440     | 43,200      | 525,600    |
| outdoor-weather     | 10 min    | 144       | 4,320       | 52,560     |
| nws-observations    | ~1 hr     | 24        | 720         | 8,760      |
| nws-forecast-hourly | periodic  | ~48       | 1,440       | 17,520     |
| outdoor-air-quality | 10 min    | 144       | 4,320       | 52,560     |
| home-assistant-state| varies    | ~500      | 15,000      | 182,500    |
| **Total**           |           | **~2,300** | **~69,000** | **~839,500** |

At these volumes:
- **First month**: ~69,000 events (if all streams active)
- **First year**: ~840,000 events
- **Multi-year (3yr)**: ~2,500,000 events

These are raw observations. For vector embeddings, not every raw point needs its own embedding. Realistic embedding counts:

| Use Case                          | Embeddings/Month | Embeddings/Year |
|-----------------------------------|-----------------|----------------|
| Every raw observation             | 69,000          | 839,500        |
| Hourly aggregates (all streams)   | 4,320           | 52,560         |
| Daily summaries (all streams)     | 180             | 2,190          |
| Anomaly events only               | ~100-500        | ~1,200-6,000   |
| Pattern signatures (rolling windows) | ~720         | ~8,760         |

**Recommendation**: Embed hourly aggregates and anomaly events, not raw observations. This brings the realistic vector count to 5,000-60,000 per year.

---

## 3. Memory Budget: Vector Storage at Each Quantization Tier

### Assumptions

- **Embedding dimension**: 384 (all-MiniLM-L6-v2 compatible, used in existing ruvector-postgres init script)
- **HNSW parameters**: M=16 (memory-constrained), ef_construction=100
- **HNSW overhead per vector**: M * 2 * 4 bytes = 16 * 2 * 4 = 128 bytes (link storage) + ~64 bytes metadata = ~192 bytes

### Per-Vector Memory

| Tier     | Vector Bytes | HNSW Overhead | Total/Vector | Compression vs f32 |
|----------|-------------|---------------|-------------|-------------------|
| f32      | 1,536 B     | 192 B         | 1,728 B     | 1x (baseline)     |
| f16      | 768 B       | 192 B         | 960 B       | 1.8x              |
| PQ8      | 48 B*       | 192 B         | 240 B       | 7.2x              |
| PQ4      | 24 B*       | 192 B         | 216 B       | 8.0x              |
| Binary   | 48 B        | 192 B         | 240 B       | 7.2x              |

*PQ8: 384 dims / 8 subspaces = 48 codebook indices (1 byte each). PQ4 similar with 4-bit codes. Codebook overhead is amortized (256 centroids * 48 dims * 4 bytes = ~49 KB per codebook, negligible at scale).

### Total Memory by Vector Count

#### 10,000 Vectors (first months of hourly aggregates)

| Tier     | Vector Data | HNSW Graph | Total  | Pi 16GB Viable? |
|----------|------------|-----------|--------|-----------------|
| f32      | 15.0 MB    | 1.9 MB    | 16.9 MB | **Yes — trivial** |
| f16      | 7.5 MB     | 1.9 MB    | 9.4 MB  | Yes            |
| PQ8      | 0.5 MB     | 1.9 MB    | 2.4 MB  | Yes            |
| Binary   | 0.5 MB     | 1.9 MB    | 2.4 MB  | Yes            |

#### 100,000 Vectors (1-2 years of hourly aggregates)

| Tier     | Vector Data | HNSW Graph | Total   | Pi 16GB Viable? |
|----------|------------|-----------|---------|-----------------|
| f32      | 150 MB     | 19 MB     | 169 MB  | **Yes — 1% of available RAM** |
| f16      | 75 MB      | 19 MB     | 94 MB   | Yes            |
| PQ8      | 5 MB       | 19 MB     | 24 MB   | Yes            |
| Binary   | 5 MB       | 19 MB     | 24 MB   | Yes            |

#### 1,000,000 Vectors (every raw observation for 1+ years)

| Tier     | Vector Data | HNSW Graph | Total    | Pi 16GB Viable? |
|----------|------------|-----------|----------|-----------------|
| f32      | 1,500 MB   | 192 MB    | 1,692 MB | **Yes — 11% of available RAM** |
| f16      | 750 MB     | 192 MB    | 942 MB   | **Yes**        |
| PQ8      | 48 MB      | 192 MB    | 240 MB   | Yes            |
| Binary   | 48 MB      | 192 MB    | 240 MB   | Yes            |

> **16GB impact:** Even 1M vectors at full f32 precision uses only 11% of available RAM. Compression (PQ8/PQ4) is optional for NDP's data volume — use f32 for simplicity and zero recall loss.

### Recall Quality at Each Tier (from ruvector documentation)

| Tier   | Recall   | Latency Overhead | Notes                              |
|--------|----------|------------------|------------------------------------|
| f32    | 99%+     | Baseline         | Full precision                     |
| f16    | 99%+     | Negligible       | Nearly identical to f32            |
| PQ8    | 90-95%   | ~1ms re-ranking  | Good for similarity search         |
| PQ4    | 85-90%   | ~1ms re-ranking  | Acceptable for anomaly detection   |
| Binary | 80-90%   | ~1ms re-ranking  | Best as pre-filter stage           |

---

## 4. Deployment Option Analysis

### Option A: RuVector PostgreSQL Extension Inside Existing TimescaleDB Container

**Approach**: Install the ruvector extension into the existing TimescaleDB (PG15) container.

**Pros**:
- Zero additional container overhead (no new process, no new memory allocation)
- Shared connection pool with Silver layer queries
- SQL-native interface -- embeddings stored alongside time-series data
- 77+ SQL functions available directly
- Existing init-scripts pattern (`deploy/pi/init-scripts/`) works perfectly
- ruvector-postgres v2.0.1 supports PG14-17 (PG15 compatible)

**Cons**:
- Current TimescaleDB container memory limit is only 256MB (shared_buffers=64MB)
- HNSW index builds will compete with TimescaleDB continuous aggregates for memory
- pgrx-based extension adds ~20-50MB resident memory
- Need to build custom Docker image (TimescaleDB + ruvector extension)
- Extension build requires pgrx 0.12, adding build complexity
- ARM64 (aarch64) support confirmed in ruvector-postgres v2.0.1 with NEON auto-detection

**Memory impact**: +20-50MB base + vector storage (see tables above)
**Container limit**: TimescaleDB is currently uncapped (using 308MB). Extension fits easily.

| Metric                | Value                               |
|-----------------------|-------------------------------------|
| Additional RAM        | 50-150 MB (10K vectors, f32+HNSW)  |
| Additional disk       | ~50 MB (extension + index files)    |
| Additional containers | 0                                   |
| Build complexity      | Medium (custom Docker image needed) |
| ARM64 support         | Confirmed (NEON auto-detected)      |

**VERDICT: VIABLE** -- Simplest option. Co-locates vectors with Gold data. No compression needed at 16GB.

---

### Option B: rvLite Embedded in NDP Rust Binary

**Approach**: Link ruvector-core (Rust crate) directly into the air-quality-app or a new dedicated binary.

**Pros**:
- 2MB binary addition (rvLite standalone size)
- ruvector-core on crates.io -- standard Cargo dependency
- SimSIMD dependency provides ARM NEON acceleration automatically
- memmap2 support for memory-mapped vector indices (critical for Pi)
- Full control over when vector operations run (batch during idle periods)
- No PostgreSQL dependency for vector operations
- Aligns with NDP's hexagonal architecture (new adapter)

**Cons**:
- Separate storage from TimescaleDB (data fragmentation)
- No SQL interface for ad-hoc vector queries from Grafana
- Need to implement embedding generation pipeline in Rust
- ruvector-core API is lower-level than SQL interface
- air-quality-app container already at 512MB limit
- REDB storage backend adds some overhead

**Memory impact**: +2MB binary + vector storage (can be mmap'd from disk)
**Key advantage**: mmap allows index to exceed available RAM by paging from NVMe

| Metric                | Value                                    |
|-----------------------|------------------------------------------|
| Additional RAM        | 2-50 MB (mmap offloads to disk)          |
| Additional disk       | Vector data size (see tables)            |
| Additional containers | 0 (embedded) or 1 (new service, ~64MB)   |
| Build complexity      | Low (Cargo dependency)                   |
| ARM64 support         | Confirmed (SimSIMD NEON, Rust aarch64)   |

**VERDICT: VIABLE** -- Best option if mmap from NVMe is available. Requires NVMe SSD (not microSD) for acceptable latency.

---

### Option C: Standalone RuVector Container (Full Features)

**Approach**: Run `ruvnet/ruvector-postgres:latest` as a separate container.

**Pros**:
- Full feature set (290+ SQL functions, GNN, attention mechanisms, hybrid search)
- Isolated from TimescaleDB -- no resource contention
- Pre-built Docker image
- Independent scaling and configuration

**Cons**:
- Unknown arm64 Docker image availability (image may be amd64 only)
- Full PostgreSQL instance = ~200-400MB additional memory
- Requires separate connection management
- Doubles PostgreSQL footprint on Pi
- The existing ruvector-postgres docker-compose sets work_mem=256MB, maintenance_work_mem=512MB -- wildly inappropriate for Pi
- Additional container overhead (PID namespace, cgroups, networking)

**Memory impact**: +200-400MB minimum (full PG instance + extension), up to ~1GB with GNN/SONA
**On 16GB Pi**: ~1GB = 6.5% of available RAM — comfortable

| Metric                | Value                                  |
|-----------------------|----------------------------------------|
| Additional RAM        | 512 MB - 1 GB (full features)          |
| Additional disk       | ~500 MB (PG data + extension)          |
| Additional containers | 1                                      |
| Build complexity      | Low (docker pull, if arm64 exists)     |
| ARM64 support         | **UNCONFIRMED** for Docker image        |

**VERDICT: VIABLE on 16GB Pi** -- Full feature set (GNN, RL, embeddings, semantic routing) now fits within budget. The 1GB allocation uses ~6.5% of available RAM. Consider this if PG extension proves limiting or if GNN/SONA features are needed.

---

### Option D: Hybrid -- rvLite Hot Path + Cloud/Server RuVector Full

**Approach**: rvLite embedded in NDP for real-time anomaly detection on Pi. Periodic sync of embeddings to a full ruvector instance on more capable hardware for complex queries (GNN, attention, hybrid search).

**Pros**:
- Minimal Pi footprint (rvLite only)
- Complex vector operations offloaded to capable hardware
- Pi handles time-critical queries locally
- Full ruvector features available for offline analysis
- Natural fit for NDP's "edge collection + cloud analysis" pattern

**Cons**:
- Requires network connectivity for sync
- Added architectural complexity
- Two systems to maintain
- Sync latency for complex queries
- Overkill if NDP never needs GNN/attention features

| Metric                | Value                               |
|-----------------------|-------------------------------------|
| Additional RAM (Pi)   | 2-50 MB                             |
| Additional disk (Pi)  | Vector data (mmap'd)                |
| Additional containers | 0 on Pi                             |
| Build complexity      | High (sync protocol, remote API)    |
| ARM64 support         | Yes (rvLite is pure Rust)           |

**VERDICT: VIABLE but premature** -- With 16GB, all ruvector features can run locally. Hybrid is only needed if a multi-Pi fleet requires coordination, which is a Phase 4 concern.

---

## 5. Embedding Generation on Pi 5

### Option 5A: LLM-Based Text Embeddings (all-MiniLM-L6-v2 via ONNX)

| Parameter           | Value                                     |
|---------------------|-------------------------------------------|
| Model               | all-MiniLM-L6-v2 (ONNX)                  |
| Model size           | ~80 MB (f32), ~43 MB (f16)               |
| Output dimension     | 384                                       |
| RAM during inference | ~100-200 MB                               |
| Latency per sentence | ~20-50 ms on Pi 5 ARM (estimated)         |
| Batch throughput     | ~20-50 sentences/sec (single core, est.)  |
| ONNX Runtime ARM     | Supported (aarch64, NEON optimized)       |

**Use case**: Embedding text descriptions, alert messages, log entries.
**NDP relevance**: Low. NDP data is primarily numerical time-series, not text.

**VERDICT: VIABLE and affordable at 16GB** -- NDP does not need text embeddings for core intelligence (sensor data is numerical). However, at 16GB the 100-200MB cost is trivial if future NL query features (Idea #5 in creative use cases) warrant it.

### Option 5B: Numerical Feature Vectors (No LLM Required)

Construct fixed-dimension vectors directly from sensor readings and engineered features. This is the natural fit for NDP's time-series data.

**Approach**: For each observation window (e.g., 1 hour), compute:

```
vector = [
    pm25_mean, pm25_std, pm25_min, pm25_max,     # 4 dims
    co2_mean, co2_std,                             # 2 dims
    temperature_mean, temperature_delta,           # 2 dims
    humidity_mean,                                 # 1 dim
    tvoc_index_mean, nox_index_mean,               # 2 dims
    hour_of_day_sin, hour_of_day_cos,              # 2 dims (cyclical encoding)
    day_of_week_sin, day_of_week_cos,              # 2 dims (cyclical encoding)
    is_weekend,                                    # 1 dim
    outdoor_temp_mean, outdoor_humidity_mean,       # 2 dims (cross-stream)
    wind_speed_mean, pressure_mean,                # 2 dims
    # Rolling features
    pm25_4h_rolling_mean, pm25_24h_rolling_mean,   # 2 dims
    co2_4h_rolling_mean,                           # 1 dim
    # Trend features
    pm25_4h_trend, co2_4h_trend,                   # 2 dims
    # Lag features
    pm25_lag_1h, pm25_lag_24h,                     # 2 dims
    co2_lag_1h,                                    # 1 dim
]
# Total: ~28-32 dimensions
```

| Parameter           | Value                          |
|---------------------|--------------------------------|
| Dimension           | 28-32 (or pad to 32/64)        |
| Computation cost    | Negligible (arithmetic only)   |
| RAM for generation  | <1 MB                          |
| Latency             | <1 ms per vector               |
| Model dependency    | None                           |

**Advantages**:
- Zero additional RAM for embedding generation
- Vectors are directly interpretable (each dimension has physical meaning)
- NDP already has feature engineering code (`core/src/forecast/features.rs`)
- Gold layer config already defines the exact aggregates and features needed
- Smaller vectors = less storage, faster search
- Perfect for anomaly detection via distance-based methods

**VERDICT: STRONGLY RECOMMENDED** -- This is the correct approach for NDP. No LLM needed. Build feature vectors from existing Gold layer aggregates.

### Option 5C: GGUF Model Inference on Pi 5

| Parameter           | Value                                    |
|---------------------|------------------------------------------|
| Smallest viable model | TinyLlama 1.1B (Q4_K_M: ~700 MB)      |
| Inference RAM        | 1-2 GB minimum                          |
| Inference speed      | ~5-15 tokens/sec on Pi 5                |
| llama.cpp ARM NEON   | Supported                               |
| ruvector GGUF support | RuvLTRA models available on HuggingFace |

**VERDICT: MARGINAL on 16GB Pi** -- TinyLlama 1.1B Q4_K_M (~700MB) would fit but leaves the Pi under heavy memory pressure if combined with all other services. Unnecessary for NDP's numerical data. If text embeddings are ever needed, use all-MiniLM-L6-v2 via ONNX instead (far lighter).

---

## 6. Disk vs Memory Tradeoffs on Pi 5

### Memory-Mapped HNSW (mmap)

ruvector-core includes `memmap2 v0.9` as an optional dependency, enabling memory-mapped vector indices.

| Storage Medium | Sequential Read | Random Read (4K) | HNSW Query Impact |
|----------------|----------------|-------------------|-------------------|
| microSD (A2)   | ~100 MB/s      | ~4,000 IOPS      | 5-50ms per query  |
| NVMe SSD       | ~800 MB/s      | ~100,000 IOPS    | <1ms per query    |
| RAM            | ~8,000 MB/s    | N/A               | <0.1ms per query  |

**Key finding**: HNSW graph traversal involves many random reads (following neighbor links). microSD random IOPS are 25x worse than NVMe. Memory-mapped HNSW on microSD would be unusable for real-time queries.

| Configuration              | Query Latency (est.) | Viable? |
|---------------------------|---------------------|---------|
| Full RAM (no mmap)         | <1 ms               | Yes     |
| mmap from NVMe SSD         | 1-5 ms              | Yes     |
| mmap from microSD          | 10-100 ms           | Marginal |
| mmap hybrid (hot in RAM)   | 1-2 ms              | Yes     |

**Pi 5 has NVMe SSD (confirmed):** Use mmap with NVMe. The 1TB NVMe provides ~100K random IOPS, sufficient for disk-backed HNSW with near-RAM performance. HNSW indices can grow without concern — mmap provides transparent overflow from RAM to NVMe.

**With 16GB RAM:** For NDP's realistic data volumes (10K-100K vectors at f32), the entire index fits in RAM with room to spare. mmap is a safety net for future growth, not an immediate necessity.

### Compression Tier Recommendation for Pi 5 (16GB + NVMe)

| Vector Count | Recommended Tier | Memory Usage | Notes |
|-------------|-----------------|-------------|-------|
| <10,000     | **f32 (no compression)** | <17 MB | Simplest, full precision |
| 10K-100K    | **f32 (no compression)** | 17-169 MB | Still <2% of available RAM |
| 100K-1M     | **f32** or f16 if preferred | 169 MB - 1.7 GB | f32 fine up to ~1M vectors |
| >1M         | PQ8 + mmap from NVMe | 240 MB+ | Only if raw observations are embedded |

> **16GB revision:** Skip compression complexity. f32 (full precision, zero recall loss) is affordable for years at NDP's data volume. Only consider PQ8 if embedding every raw observation (1M+/year).

---

## 7. ARM Cross-Compilation

### Rust -> aarch64-unknown-linux-gnu

NDP already builds Rust binaries for Pi (the entire platform runs on Pi 5). The cross-compilation path is established:

```
# Already in NDP's workflow
rustup target add aarch64-unknown-linux-gnu
cargo build --target aarch64-unknown-linux-gnu --release
```

Or via Docker multi-stage build (current approach per `Dockerfile`).

### ruvector-core ARM Optimization

| Component        | ARM Support              | Notes                          |
|-----------------|--------------------------|--------------------------------|
| ruvector-core   | aarch64 confirmed        | Pure Rust + SimSIMD            |
| SimSIMD         | NEON, SVE, SVE2          | Auto-detected at runtime       |
| hnsw_rs         | Platform-independent     | Pure Rust                      |
| memmap2         | Platform-independent     | Kernel mmap support            |
| REDB            | Platform-independent     | Pure Rust B-tree storage       |
| pgrx (PG ext)   | aarch64 confirmed       | PostgreSQL extension framework |

SimSIMD NEON kernels provide hardware-accelerated distance computation on Pi 5's Cortex-A76 cores. Expected speedup: 4-8x over scalar computation for f32 vectors, even larger for quantized types.

### Docker Multi-Arch Build

For the PostgreSQL extension approach, a custom Docker image is needed:

```dockerfile
# Conceptual: TimescaleDB + ruvector extension
FROM timescale/timescaledb:latest-pg15
# Install ruvector extension (pre-compiled for aarch64)
# This requires building ruvector-postgres with pgrx for pg15 on aarch64
```

The `timescale/timescaledb:latest-pg15` image supports both amd64 and arm64, so the base is confirmed. The ruvector-postgres extension needs to be compiled for aarch64/pg15 during the Docker build.

---

## 8. Resource Sharing with TimescaleDB

If using the PostgreSQL extension approach (Option A):

### Shared Memory Implications

| Resource            | Current (TimescaleDB) | With ruvector (est.) | Notes                    |
|--------------------|-----------------------|---------------------|--------------------------|
| shared_buffers      | 64 MB                 | 64 MB (unchanged)   | Extension uses PG buffers |
| work_mem            | 16 MB                 | 16 MB (unchanged)   | Per-operation memory      |
| HNSW index in cache | 0                     | 2-20 MB             | Cached in shared_buffers  |
| Extension resident  | 0                     | 20-30 MB            | ruvector code + state     |
| Background workers  | 2 (TimescaleDB)       | 2-3                 | +1 for HNSW maintenance   |

**Total additional memory**: ~40-70 MB within the PG process.
**Required container limit**: 384-512 MB (up from current 256 MB).

### Connection Pooling

Current configuration: `max_connections = 20`. ruvector operations use the same connection pool. No additional connections needed -- vector queries are just SQL.

### Background Worker Overhead

ruvector may use background workers for:
- HNSW index auto-maintenance
- Quantization level promotion/demotion
- Index compaction

Current `max_worker_processes = 4` should accommodate 1 additional ruvector worker. Consider increasing to 5.

---

## 9. Recommended Deployment Path

### Phase 1: Numerical Feature Vectors + PG Extension (Immediate)

1. **Generate embeddings**: Build 28-39 dimensional feature vectors from Gold layer aggregates. No LLM required. Use f32 (no compression). Leverage existing `core/src/forecast/features.rs` code.

2. **Store via ruvector PG extension**: Add ruvector to the existing TimescaleDB container.
   - Build custom Docker image: `timescaledb:latest-pg15` + ruvector extension
   - Add init script similar to existing `deploy/pi/init-scripts/` pattern
   - Use native dimension (28-39 dim) — no need to pad to 384
   - HNSW with M=16, ef_construction=200 (can afford higher ef at 16GB)

3. **Memory budget**: +50-100 MB within TimescaleDB process. Trivial at 16GB.

4. **Use cases enabled**:
   - Sensor fingerprinting: "what situation is this?"
   - Anomaly detection: "find hours similar to this anomalous hour"
   - K-NN similarity search: "find historical periods with similar sensor signatures"
   - Causal discovery: "which sensor patterns co-occur with high PM2.5?"
   - Seed causal knowledge graph with initial stream relationships
   - Forecast input: feature vectors feed directly into ruv-FANN models

5. **Decision point**: If PG extension proves limiting (no GNN, no RL hooks), deploy full ruvector container (~1GB)

### Phase 2: rvLite for Hot-Path Queries (When Needed)

If sub-millisecond vector queries are needed (e.g., real-time anomaly alerting):

1. Add `ruvector-core` as Cargo dependency to air-quality-app or a new crate
2. Maintain a small in-memory index of recent patterns (last 7 days)
3. Use PQ8 quantization for the in-memory index
4. Sync to PostgreSQL for long-term storage

### Phase 3: Full Container + Advanced Features (If PG Extension Proves Limiting)

With 16GB, a full ruvector container is viable from Phase 1, but may be deferred for simplicity:

1. Deploy ruvector container with 1GB memory limit (~6.5% of available)
2. Enable GNN module for causal knowledge graph (~320MB)
3. Enable SONA for model tournament with LoRA adapters (~200MB)
4. Enable ReasoningBank for decision trajectory recording (~50MB)
5. Temporal and cross-stream attention exploration

### Phase 4: Text Embeddings + Federation (If Ever Needed)

1. Add all-MiniLM-L6-v2 ONNX model (~80-200 MB) for natural language query features
2. Federated learning across Pi fleet via binary vector sync
3. Cross-domain transfer via hyperbolic embeddings

---

## 10. Summary Verdicts (Revised for 16GB + NVMe)

| Option | Description | Pi 16GB + NVMe | Recommended? |
|--------|-------------|----------------|-------------|
| A | PG Extension in TimescaleDB | **VIABLE** | **YES (Phase 1 — simplest start)** |
| B | rvLite embedded in Rust binary | **VIABLE** | Alternative to A for hot-path queries |
| C | Standalone ruvector container | **VIABLE (~1GB)** | **YES if GNN/SONA needed** |
| D | Hybrid rvLite + remote full | VIABLE | Phase 4 (federation only) |

| Embedding Strategy | Pi 16GB | Recommended? |
|-------------------|---------|-------------|
| Numerical feature vectors | **VIABLE** | **YES (immediate, core approach)** |
| all-MiniLM-L6-v2 ONNX | **VIABLE** | Optional — if NL query features wanted |
| GGUF LLM inference | MARGINAL | No — unnecessary for NDP |

| Compression Strategy | Pi 16GB | Recommended? |
|---------------------|---------|-------------|
| f32 (no compression) | **VIABLE for years** | **YES — simplest, full precision** |
| PQ8 | VIABLE | Only if >1M vectors |
| Binary | VIABLE | Only for federation sync |

---

## 11. Critical Risks and Mitigations (Revised)

| Risk | Impact | Mitigation |
|------|--------|------------|
| ruvector-postgres Docker image not available for arm64 | Blocks Option A | Build from source using pgrx. Confirmed aarch64 support in crate. |
| HNSW index build exhausts memory during bulk insert | OOM kill | Build index incrementally, insert in batches of 1000. At 16GB this is very unlikely but still good practice. |
| ~~microSD IOPS insufficient for mmap~~ | ~~Slow queries~~ | **RESOLVED:** NVMe SSD confirmed (~100K random IOPS). mmap queries in 1-5ms range. |
| TimescaleDB continuous aggregates contend with vector queries | Query latency | Schedule vector index maintenance during low-activity periods (2-5 AM) |
| ~~256MB container limit insufficient~~ | ~~Extension cannot load~~ | **RESOLVED:** TimescaleDB container is uncapped (using 308MB). Extension fits easily. |
| Over-engineering for current data volume | Wasted complexity | Start with PG extension (simplest). Evaluate full container only if GNN/SONA proves valuable. |

---

## Sources

- [RuVector GitHub Repository](https://github.com/ruvnet/ruvector)
- [RuVector Technical Specification (Gist)](https://gist.github.com/ruvnet/f9b631bae8303cb114bd7bf3a8e39217)
- [ruvector-core on crates.io](https://crates.io/crates/ruvector-core)
- [ruvector-core API Documentation](https://docs.rs/ruvector-core/latest/ruvector_core/)
- [ruvector-postgres on crates.io (v2.0.1)](https://docs.rs/crate/ruvector-postgres/latest)
- [ruvnet/ruvector-postgres Docker Hub](https://hub.docker.com/r/ruvnet/ruvector-postgres)
- [SimSIMD - SIMD Distance Computation](https://github.com/ashvardanian/SimSIMD)
- [HNSW Memory Overhead (Milvus FAQ)](https://milvus.io/ai-quick-reference/how-much-memory-overhead-is-typically-introduced-by-indexes-like-hnsw-or-ivf-for-a-given-number-of-vectors-and-how-can-this-overhead-be-managed-or-configured)
- [HNSW Algorithm Parameters (nmslib)](https://github.com/nmslib/hnswlib/blob/master/ALGO_PARAMS.md)
- [pgvector HNSW Indexes (Crunchy Data)](https://www.crunchydata.com/blog/hnsw-indexes-with-postgres-and-pgvector)
- [all-MiniLM-L6-v2 (Hugging Face)](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)
- [Time Series Embeddings for Anomaly Detection (Springer)](https://link.springer.com/article/10.1007/s41019-025-00295-w)
- [Cross-Compiling Rust for ARM (Docker Blog)](https://www.docker.com/blog/cross-compiling-rust-code-for-multiple-architectures/)
