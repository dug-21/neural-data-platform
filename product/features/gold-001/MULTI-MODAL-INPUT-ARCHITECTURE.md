# Multi-Modal Input Architecture: Beyond Numeric Sensors

> **Date:** 2026-02-11
> **Type:** Architectural exploration (discussion artifact, not specification)
> **Context:** Emerged from roadmap v1.2 discussion about domain generality
> **Status:** Exploration — captures current thinking, not commitments

---

## Origin

The intelligence layer roadmap (v1.1) was designed exclusively for **numeric metric streams** — sensor readings flowing through Gold continuous aggregates into z-score normalized embedding vectors. During architectural review, a question was raised:

> "What happens when we apply this platform to sysops/observability? Docker logs are text, not numbers."

This led to a deeper exploration: the platform claims to be a generic data platform, but the intelligence pipeline only handles one data modality. If a second domain requires text processing and a future domain might involve images, the architecture needs to support multiple input types — or at least not preclude them.

This document captures that exploration and proposes an architecture that handles it.

---

## The Two Stream Types

NDP encounters two fundamentally different data shapes:

### Metric Streams (Numeric)

Structured numerical readings at regular intervals. The original and well-understood case.

```
Source → Bronze (Parquet) → Silver (hypertable) → Gold (CA) → Embedding (z-score → Vec<f32>)
```

- **Examples:** PurpleAir sensor readings, NWS observations (numeric fields), Docker container CPU/memory stats, system load/disk/temperature
- **Embedding method:** Z-score normalize selected fields, concatenate into Vec<f32>
- **Dimensionality:** ~28-64 depending on field count
- **Volume:** 1 embedding per time bucket (hourly). Tiny storage.
- **Dependencies:** None beyond basic math

### Event Streams (Text-Based)

Timestamped text payloads arriving as discrete events, often irregular.

```
Source → Bronze (Parquet) → Silver (hypertable) → Gold (text features + embedding)
                                                         ↓
                                                   Template cache → MiniLM → Vec<f32> [384D]
```

- **Examples:** NWS forecast discussions, AirNow forecast narratives, NWS alerts, Docker log lines, systemd journal entries
- **Embedding method:** Language model (all-MiniLM-L6-v2 via ONNX) produces 384-dimension vectors. Template caching avoids redundant inference.
- **Dimensionality:** 384 (MiniLM output), optionally PCA-reduced to 16-32 for composite embeddings
- **Volume:** Variable. NWS forecasts = ~8/day (trivial). Docker logs = ~50K/day (requires quantization and retention tiers).
- **Dependencies:** ONNX Runtime, MiniLM model (~80MB ONNX / ~200MB in memory)

---

## The Observation That Led Here

During discussion of the sysops/observability domain, a key insight emerged:

> Reducing log lines to `error_count_1h` is only numeric if each error type has a number. "12 connection-refused errors" is a completely different situation than "12 OOM-kills," even though both produce `error_count = 12`.

This means the sensor domain is "easy mode" for embeddings — everything is already numeric with known dimensions. Domains with text data force a harder problem: representing semantic content as numbers.

The follow-up observation:

> Log lines ARE events. They have a timestamp, a source, and a payload. The payload is text instead of a number, but the stream infrastructure is the same.

This reframed the problem from "how do we handle text" to "the platform has two kinds of streams, and needs two embedding paths."

---

## The Embedder Trait

The core abstraction that makes the pipeline modality-agnostic:

```rust
pub trait Embedder: Send + Sync {
    fn embed(&self, row: &GoldRow) -> Result<Embedding>;
    fn dimensions(&self) -> usize;
    fn name(&self) -> &str;
}
```

Three implementations serve current and near-future needs:

| Implementation | Input | Output | Dependencies |
|---------------|-------|--------|-------------|
| `MetricEmbedder` | Numeric Gold fields | Vec<f32> [~32D] | None (pure math) |
| `EventEmbedder` | Text events | Vec<f32> [384D] | MiniLM ONNX model |
| `CompositeEmbedder` | Both | Vec<f32> [~48-64D] | Both sub-embedders |

The stream config determines which embedder is used. The intelligence layer (search, predict, learn) doesn't know or care what produced the vector — it's always Vec<f32> into HNSW.

---

## Template Caching for Text Efficiency

A running system produces a finite, relatively stable set of text patterns. NWS forecasts repeat structural templates ("Partly cloudy with a high near {N}"). Docker logs repeat message templates ("Connection to {host}:{port} refused").

Template caching exploits this:

1. New text event arrives
2. Normalize (lowercase, strip numbers/IPs/timestamps)
3. Compare normalized form against template cache (cosine similarity > 0.95)
4. **Cache hit:** Reuse the template's embedding (zero inference cost)
5. **Cache miss:** Run MiniLM inference, cache the new template

After warmup (typically minutes), cache hit rates exceed 90% for operational systems. The MiniLM model is effectively idle most of the time.

**Cache sizing:** A typical system produces 50-200 unique templates. At 384D × f32 per template, the entire cache is <300KB.

---

## Quantization: When and Why

Quantization compresses vectors for storage efficiency, trading a small recall loss for significant space savings.

### When It Matters

| Stream Type | Volume | f32 Storage | PQ8 Storage | Need Quantization? |
|-------------|--------|------------|------------|-------------------|
| Metric (hourly, 32D) | 8,760/yr | 1.1 MB/yr | 0.3 MB/yr | No |
| Text (AQ forecasts, 384D) | ~3K/yr | 4.4 MB/yr | 1.1 MB/yr | No |
| Text (sysops logs, 384D) | ~18M/yr | 27 GB/yr | 6.8 GB/yr | **Yes** |
| Text (high-volume logs, 384D) | ~100M/yr | 150 GB/yr | 37 GB/yr | **Mandatory** |

Quantization is irrelevant for metric streams and low-volume event streams. It becomes essential when per-event text embeddings accumulate at high rates.

### Available Methods (from ruvector-core)

| Method | Compression | Typical Recall | Best For |
|--------|------------|---------------|----------|
| f32 (none) | 1× | 100% | Low volume, precision critical |
| Scalar (int8) | 4× | >99% | Medium volume |
| PQ8 | 4× | ~95-98% | High volume, good recall |
| PQ4 | 8× | ~90-95% | Very high volume |
| Binary | 32× | ~80-90% | Extreme volume, coarse filtering |

### Config-Driven Selection

```yaml
streams:
  purpleair:
    intelligence:
      quantization: none       # 1 MB/year, why bother

  nws-forecast-hourly:
    intelligence:
      quantization: none       # 4 MB/year, why bother

  docker-logs:
    intelligence:
      quantization: pq8        # 27 GB → 6.8 GB/year
      retention:
        hot: 24h               # All events (19 MB/day PQ8)
        warm: 30d              # Anomalous only
        cold: forever          # Centroids only
```

---

## Tiered Retention for Event Embeddings

Event-level embeddings accumulate much faster than hourly metric embeddings. Tiered retention manages the lifecycle:

### Three Tiers

| Tier | Duration | What's Kept | Purpose |
|------|----------|------------|---------|
| **Hot** | 24 hours | All individual event embeddings | "Have I seen this exact error before?" |
| **Warm** | 30 days | Only anomalous/error events | "What unusual events preceded past incidents?" |
| **Cold** | Forever | Hourly centroid embeddings only | "What was the overall character of that time period?" |

### The Centroid Preserves Long-Term Memory

When individual event embeddings age out, their centroid (mean vector of all events in the time bucket) persists. This captures "what kind of events were happening" without storing every event individually.

A centroid dominated by "connection refused" errors lives in a different region of embedding space than one dominated by "OOM killed" events. The K-NN search on centroids still distinguishes these situations — just with less granularity than searching individual events.

---

## Air Quality Text: Dogfooding the Pipeline

Rather than deferring text embeddings to a future sysops domain, we exercise the pipeline in V1.2 using air quality text data that already exists:

### NWS Forecast Discussions

NWS provides `detailedForecast` text with each forecast period:

> "A stagnation advisory is in effect through Thursday. Light winds and temperature inversions will trap pollutants near the surface. Air quality may deteriorate to Unhealthy for Sensitive Groups."

These forecasts have direct predictive value for sensor readings. Embedding them alongside metrics creates **forecast-aware similarity search**: "Find past hours where sensors looked like now AND the forecast context was similar."

This distinguishes:
- "High PM2.5 + stagnation advisory" (will get worse) from
- "High PM2.5 + incoming front" (about to improve)

Both have the same metric embedding; the event embedding differentiates them.

### NWS Alerts/Advisories

Text events with direct air quality implications (wildfire smoke warnings, stagnation advisories). These are irregular, event-driven — exactly the pattern sysops logs will have.

### What This Proves

| Capability | Validated By |
|-----------|-------------|
| EventEmbedder end-to-end | NWS text → MiniLM → vector → HNSW → search |
| Template caching | NWS forecasts are structurally repetitive |
| CompositeEmbedder | Metric + forecast text combined search |
| Quantization | PQ8 on forecast embeddings, recall comparison |
| Tiered retention | Hot/warm/cold lifecycle (low volume exercises the machinery) |
| Config-driven pipeline | `embedding_type: event` in stream config |

---

## Sysops/Observability as Second Domain

The second domain validates generality. Sysops data maps to the same pipeline:

### Stream Mapping

| Sysops Source | Stream Type | NDP Equivalent |
|--------------|-------------|---------------|
| `docker stats` output | Metric | Like PurpleAir sensor readings |
| `journalctl` entries | Event | Like NWS forecast discussions |
| Container log lines | Event | Like NWS alerts |
| System metrics (CPU, disk, temp) | Metric | Like NWS observations |

### Embedding Comparison

| Dimension | Air Quality | Sysops |
|-----------|------------|--------|
| Temporal | hour, day, weekend | hour, day, weekend |
| Core metrics | PM2.5, CO2, temp, humidity | CPU%, memory%, disk%, load |
| Derived | trends, rolling stats | memory trend, IO rate |
| State events | window/door transitions | container restarts, health failures |
| Text context | NWS forecast centroid | Log line centroid |
| **Total** | ~48D composite | ~50-60D composite |

### What Must NOT Require Code Changes

For sysops to validate the "config-driven" claim, adding it must require only:
1. New stream configs (metric + event)
2. New Silver hypertable schemas
3. New Gold aligned view DDL (generated from config)
4. New domain intelligence config (embedding type, quantization, retention)
5. **Zero changes to ndp-intelligence crate or binary**

If this fails — if sysops needs intelligence code changes — the architecture has a gap.

---

## Could This Extend Further?

The Embedder trait produces Vec<f32> from any input. Text is the immediate next modality beyond numbers. But the trait doesn't preclude others.

### Image Embeddings (Speculative)

**What it would look like:** A Pi Camera captures images of the environment. An image embedding model (CLIP ViT-B/32 or similar) produces a vector. The vector goes into the same HNSW index.

**Use cases (speculative):**
- Visual occupancy detection (room occupied → higher CO2 expected)
- Window/door state from image (supplement binary sensors)
- Environmental condition classification (smoke visible, condensation on windows)
- Platform self-monitoring (is the LED pattern on the Pi normal?)

**Feasibility on Pi 5:**
- CLIP ViT-B/32: ~340MB model, ~100ms inference on ARM
- Within the 16GB budget, but adds meaningful memory pressure
- Would need its own `ImageEmbedder` implementing the same trait
- Image capture rate would be low (1/minute or less), so volume isn't a concern

**Assessment:** Technically viable on the hardware. The Embedder trait supports it. But there's no current requirement, and the complexity of image processing, camera integration, and model management is significant. Worth noting as architecturally possible but not worth designing for today.

### Audio Embeddings (Speculative)

**What it would look like:** A USB microphone captures environmental sound. An audio classification model produces a vector.

**Use cases (speculative):**
- Environmental noise classification (traffic, construction, HVAC running)
- Correlation with air quality (heavy traffic → higher PM2.5)
- Anomaly detection (unusual sounds in the home)

**Feasibility on Pi 5:**
- Audio classification models (YAMNet): ~20MB, fast inference
- Much lighter than image models
- Real-time audio processing is well-supported on Pi

**Assessment:** Lighter than images and potentially valuable for environmental correlation. Same Embedder trait pattern. But far from any current requirement.

### Structured Event Payloads (Near-Term)

**What it would look like:** JSON payloads from webhooks, Home Assistant events, MQTT messages with structured content.

```json
{"event": "motion_detected", "room": "living_room", "confidence": 0.92}
```

**How it fits:** These are already partially handled — structured numeric fields go through MetricEmbedder. But the categorical fields (`event`, `room`) are text. A lightweight approach: map categorical values to learned embeddings (like word2vec for categories), no heavy model needed.

**Assessment:** This is the most practical near-term extension. Home Assistant events are already an NDP data source. Better categorical handling would improve metric embeddings without requiring a full text model.

---

## The Architecture Decision

The Embedder trait is the key abstraction. It:

1. **Decouples input modality from intelligence** — search/predict/learn code is identical regardless of what produced the vector
2. **Makes modalities config-driven** — stream config specifies `embedding_type: metric | event | composite`
3. **Allows incremental capability addition** — new Embedder implementations can be added without modifying existing code
4. **Keeps the default path lightweight** — pure-metric domains never load MiniLM or any heavy model

The architecture explicitly supports:
- **Today:** Metric streams (z-score numeric)
- **V1.2:** Event streams (MiniLM text) dogfooded with NWS forecasts
- **V1.3:** Sysops domain exercising event streams at scale
- **Future:** Any modality that can produce Vec<f32> from its input

What it explicitly does NOT do:
- No multi-modal fusion (combining image + text + metric into a single model). Each modality produces its own embedding; CompositeEmbedder concatenates them.
- No training custom embedding models. Uses pre-trained models (MiniLM, potentially CLIP) with optional SONA fine-tuning downstream.
- No real-time streaming embeddings. All embedding happens in batch at the Gold layer's cadence (15-minute cycles).

---

## Impact on Platform Identity

This exploration revealed something about NDP's identity. The platform was built for "air quality monitoring on a Pi." But the architecture — Bronze → Silver → Gold → Intelligence, with config-driven stream definitions and the Embedder trait — is genuinely domain-agnostic.

Air quality is the first domain. Sysops is the second. The intelligence layer doesn't know or care about the difference. What changes per domain:
- Stream configs (what data to ingest)
- Gold views (how to align and aggregate)
- Embedding configs (what type, what fields, what quantization)

What doesn't change:
- The pipeline (Bronze → Silver → Gold → Embeddings → HNSW → Search → Predict)
- The intelligence library (K-NN, Granger, SONA, predictions, outcomes)
- The binary (daemon, PG NOTIFY, CLI modes)
- The MCP query interface ("what's happening?")

The Pi becomes less of a "sensor monitor" and more of an **environmental and operational intelligence appliance** — it understands its environment (air quality), itself (sysops), and the interactions between them (cross-domain correlations), all through the same configurable pipeline.

Whether that extends to images, audio, or other modalities is an open question. The architecture doesn't prevent it. But the immediate value is in proving the two modalities we know we need: metrics and text.

---

## Summary

| Topic | Decision |
|-------|----------|
| Two stream types | Metric (numeric) + Event (text). Both produce Vec<f32>. |
| Abstraction | Embedder trait with MetricEmbedder, EventEmbedder, CompositeEmbedder |
| Text model | all-MiniLM-L6-v2 via ONNX Runtime. 384D output. ~200MB on demand. |
| Efficiency | Template caching. >90% hit rate after warmup. Model mostly idle. |
| Scale | Quantization (PQ8) for high-volume event streams. Config-driven per stream. |
| Storage | Tiered retention (hot/warm/cold) for event embeddings. Centroids permanent. |
| Dogfooding | NWS forecast text exercises event pipeline in V1.2, within air quality domain. |
| Second domain | Sysops in V1.3 validates generality. Zero intelligence code changes required. |
| Future modalities | Images (CLIP, speculative), audio (YAMNet, speculative), structured events (near-term). Architecturally possible via Embedder trait. No current requirement. |
| Config-driven | Stream config determines embedding type, model, quantization, retention. |

---

*This document captures architectural exploration from roadmap v1.2 discussion. It is a discussion artifact, not a specification. Implementation decisions will be made during SPARC phases for the relevant features.*
