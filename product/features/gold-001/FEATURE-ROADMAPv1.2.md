# Gold Layer Feature Roadmap v1.2: Domain-Agnostic Intelligence Platform

> **Supersedes:** FEATURE-ROADMAPv1.1.md
> **Created:** 2026-02-11
> **Method:** Working backwards from V2.0, augmented with ruvector research + multi-modal input analysis
> **Status:** Draft for Review
> **Research Basis:** `product/research/ruvector/` (6 documents), `MULTI-MODAL-INPUT-ARCHITECTURE.md`
> **Prior versions:** `FEATURE-ROADMAP.md` (v1.0), `FEATURE-ROADMAPv1.1.md` (v1.1)

---

## What Changed From v1.1 and Why

v1.1 integrated ruvector as a **sensor intelligence layer** — numerical embeddings of Gold aggregates, K-NN search, Granger validation. That architecture is correct but incomplete.

Two realizations emerged from architectural discussion:

### 1. The Platform Must Prove Generality, Not Just Claim It

v1.1 framed V2.0 as "add a new domain via config." But if V1.2 only exercises numerical metric embeddings, we don't know whether the intelligence pipeline generalizes to domains with **text data** (logs, forecasts, alerts) until V2.0. That's too late.

**Fix:** Dogfood text embeddings in V1.2 using air quality data that's already available — NWS forecast discussions, AirNow forecast narratives, NWS alerts. These are legitimate text event streams within the air quality domain. By proving text embeddings work alongside metric embeddings in V1.2, we know the architecture generalizes before claiming it does.

### 2. There Are Two Kinds of Streams

v1.1 treated all data as numeric metrics flowing through Gold continuous aggregates. But NDP will encounter:

| Stream Type | Data Shape | Embedding Method | Example |
|-------------|-----------|-----------------|---------|
| **Metric** | Structured numbers at regular intervals | Z-score normalize, build Vec<f32> directly | Sensor readings, container CPU/memory |
| **Event** | Timestamped text/structured payloads, irregular | Template cache + text embedding model → Vec<f32> | Log lines, forecast narratives, alerts |

Both produce the same output type (a vector for HNSW), so the intelligence layer's search/predict/learn code is identical. But the **embedding pipeline** needs two paths, abstracted behind a trait.

### 3. Feature Engineering ≠ Intelligence

v1.1 bundled everything from embedding through prediction into `ndp-intelligence-app`. But the architectural boundary between **preparing data for analysis** and **performing analysis** was blurred.

The Gold layer already IS the feature engineering layer — continuous aggregates, lag features, rolling stats, trends, aligned views. These are all data preparation, not intelligence. The embedding pipeline (Gold row → normalized Vec<f32> → pgvector) is the same category of work: transforming data into a shape useful for the next consumer.

Similarly, text feature extraction — parsing log severity, extracting keywords, computing sentiment flags — is feature engineering. It produces structured numeric fields from unstructured text. That's the same job as a continuous aggregate producing `AVG(pm25)` from raw readings.

**Intelligence starts at SEARCH.** That's where the system makes a judgment: "this situation resembles that past situation, and here's what happened next."

| Step | Category | Where It Lives |
|------|----------|----------------|
| Gold CAs, features, aligned view | Feature engineering | `ndp-lib::gold` (existing) |
| Text feature extraction | Feature engineering | `ndp-lib::gold` (new generator) |
| Metric embedding (z-score → Vec<f32>) | Feature engineering | `ndp-lib::gold` (new) |
| Event embedding (MiniLM → Vec<f32>) | Feature engineering | `ndp-lib::gold` (new) |
| Composite embedding (combine both) | Feature engineering | `ndp-lib::gold` (new) |
| pgvector storage | Data platform (storage) | `ndp-lib::gold` |
| K-NN search | **Intelligence** | `ndp-intelligence` |
| Prediction generation | **Intelligence** | `ndp-intelligence` |
| Granger validation | **Intelligence** | `ndp-intelligence` |
| SONA learning | **Intelligence** | `ndp-intelligence` |

This separation means:
- `ndp-cli` can run feature engineering without intelligence (`ndp gold populate`, `ndp gold embed`)
- Text extraction iterates independently of the intelligence pipeline
- Intelligence becomes a pure consumer of prepared features — swap the search algorithm without touching embeddings
- Dashboard-only use cases (observability) work without the intelligence binary running

### 4. Text Feature Extraction: Hybrid Approach

Text data (logs, forecast narratives, alerts) can't flow through TimescaleDB continuous aggregates — CAs only support aggregate functions, not regex or keyword extraction. Three options were evaluated:

| Option | Mechanism | Pros | Cons |
|--------|-----------|------|------|
| SQL intermediate view | `CREATE MATERIALIZED VIEW` with CASE/regex | Stays in DDL pipeline | Hard to iterate; limited SQL text functions |
| Rust extraction, write to Gold | Rust code extracts features, writes to Gold hypertable | Flexible, testable, iteratable | Needs a populator step |
| Intelligence-only | Extract at embedding time, never persisted | Fastest to ship | Not queryable in Grafana, not available for Granger |

**Decision: Hybrid (Rust extraction, Gold table storage).** The DDL generator creates the target table schema (config-driven). Rust code extracts text features and populates the table. Results are SQL-queryable, joinable into aligned views, visible in Grafana.

This follows the same pattern the intelligence service already uses: read from Gold, compute in Rust, write results back to Gold tables (`gold.predictions`, `gold.sensor_embeddings`). The difference is that text extraction is feature engineering, so the extraction code lives in `ndp-lib::gold` alongside other Gold generators, not in the intelligence crate.

```json
{
  "stream_id": "system-logs",
  "gold_etl": {
    "extracted_features": {
      "enabled": true,
      "source": "rust",
      "fields": {
        "severity_score": "INTEGER",
        "contains_timeout": "BOOLEAN",
        "error_category": "TEXT",
        "message_length": "INTEGER"
      }
    }
  }
}
```

The DDL generator produces a plain hypertable (not a CA):

```sql
CREATE TABLE IF NOT EXISTS gold.system_logs_text_features (
    bucket         TIMESTAMPTZ NOT NULL,
    ndp_id         TEXT NOT NULL,
    severity_score INTEGER,
    contains_timeout BOOLEAN,
    error_category TEXT,
    message_length INTEGER,
    PRIMARY KEY (bucket, ndp_id)
);
SELECT create_hypertable('gold.system_logs_text_features', 'bucket',
    if_not_exists => TRUE);
```

The `AlignedViewGenerator` joins this table like any other Gold source. The intelligence embedding pipeline consumes the extracted features as regular numeric dimensions.

### 5. Quantization Matters for Scale

v1.1 said "skip compression, f32 fine for years." True for hourly metric embeddings (~1MB/year). Not true for per-event text embeddings in high-volume domains:

| Stream | Volume | f32 (384D) | PQ8 (384D) | Ratio |
|--------|--------|-----------|-----------|-------|
| Metric (hourly) | 8,760/yr | 1.1 MB | 0.3 MB | Compression pointless |
| Text (AQ forecasts) | ~3K/yr | 4.4 MB | 1.1 MB | Compression pointless |
| Text (sysops logs) | ~18M/yr | 27 GB | 6.8 GB | **Compression required** |

V1.2 should implement and validate quantization even though air quality doesn't need it — because the next domain will.

### Summary of Changes

| Aspect | v1.1 | v1.2 |
|--------|------|------|
| Framing | Sensor intelligence | Domain-agnostic intelligence platform |
| Stream types | Metric only | Metric + Event |
| Text embeddings | Explicitly excluded | Dogfooded with NWS forecasts |
| Embedding pipeline | Hardcoded numeric | Embedder trait (Metric, Event, Composite) |
| Architectural boundary | Blurred (all in intelligence) | Feature engineering = Gold layer; intelligence = pure consumer |
| Text feature extraction | Not addressed | Hybrid: Rust extraction → Gold hypertable (config-driven schema) |
| Quantization | Deferred (unnecessary) | Validated in V1.2 (config-driven, per-stream) |
| Retention | Unlimited (small data) | Tiered hot/warm/cold (config-driven, per-stream) |
| V1.3 scope | SONA + actions | SONA + actions + sysops as second domain |
| V2.0 scope | Multi-stream | Multi-domain (streams + text + cross-domain) |
| MiniLM model | Not included | +200MB, loaded on demand for event streams |

---

## Updated Vision

### The Capability Chain (Revised)

```
V1.0          V1.1           V1.2                    V1.3                V2.0
-----         ----           ----                    ----                ----
Ingest   ->   Prepare    ->  Detect + Remember   ->  Predict/Act     ->  New Domain
Data          for Detection  Metrics AND Text        with Learning       via Config Only
                             Prove Generality        Second Domain

Bronze->Silver Gold Layer    Domain-Agnostic         SONA + Sysops       Multi-Domain
Pipeline       Foundation    Intelligence Engine     Validation           Intelligence
                             (metrics + events)      (proves config)
```

### The Key Insight (Updated)

V1.2 is "build a memory of situations AND a memory of events." Metric streams tell the platform **what's happening** (CO2 = 650, humidity = 70%). Event streams tell it **what's being said** ("Stagnation advisory in effect through Thursday"). The combination — numeric state plus textual context — is richer than either alone.

And by proving both paths work within the air quality domain, V1.3's sysops domain becomes a configuration exercise rather than an architecture exercise.

### The Dogfooding Principle

Every foundational capability MUST be validated in V1.2 using air quality data:

| Capability | Air Quality Exercise | Future Domain Exercise |
|-----------|---------------------|----------------------|
| MetricEmbedder | Sensor state vectors (existing plan) | Container CPU/memory metrics |
| EventEmbedder | NWS forecast text, AirNow narratives | Docker log lines, journal entries |
| Template caching | NWS discussion templates | Log message templates |
| Quantization (PQ8) | Validate on forecast text (unneeded at AQ scale) | Required for sysops event volume |
| Tiered retention | Validate hot/warm/cold lifecycle | Required for sysops event volume |
| Config-driven pipeline | Stream config drives embedding type | Same config schema, different streams |
| K-NN similarity | "Similar past air quality states" | "Similar past operational states" |
| Composite embedding | Sensor metrics + forecast text centroid | Container metrics + log text centroid |

If it doesn't work for air quality, it won't work for anything else. Fix it here.

---

## Two Stream Types

### Metric Streams (Numeric)

The approach from v1.1, unchanged. Gold continuous aggregates produce hourly rows with numeric fields. The embedding pipeline z-score normalizes and builds a Vec<f32> directly.

```
Gold aligned view row (numeric) → z-score normalize → Vec<f32> [~28-32D]
```

Examples: PurpleAir sensor readings, NWS observations (numeric fields), Home Assistant state transitions.

### Event Streams (Text-Based)

New in v1.2. Text payloads that arrive as discrete events, embedded using a language model, with template caching for efficiency.

```
Text event → template match? → cache hit: reuse embedding
                              → cache miss: MiniLM embed → cache → Vec<f32> [384D]
```

Examples: NWS forecast discussions, AirNow forecast narratives, NWS alerts/advisories.

### How They Combine

For any time bucket, the Gold layer can produce a **composite embedding** that combines numeric state with textual context:

```
[metric_embedding (~32D)] + [event_centroid (PCA-reduced to ~16-32D)] = composite [~48-64D]
```

The event centroid is the mean of all event embeddings in the time bucket, reduced via PCA to manageable dimensions. This captures "what was the textual context during this hour?" alongside "what were the metrics?"

K-NN search on composite embeddings finds hours where both the sensor readings AND the forecast context were similar — a much more specific match than metrics alone.

---

## What Actually Calls Ruvector

### The Runtime Architecture

Unchanged from v1.1 — intelligence runs as a **separate process** from ingestion. See v1.1 for full workload profile comparison.

```
Sources (MQTT/HTTP) ──> air-quality-app ──> Bronze (Parquet)
                                        ──> Silver (TimescaleDB)
                                                    │
                                          Gold CAs refresh (15 min)
                                                    │
                                                    ▼
                              ┌─────────────────────────────────────────────┐
                              │          TimescaleDB (PostgreSQL 15)         │
                              │                                              │
                              │  pgvector extension (durable vectors)       │
                              │                                              │
                              │  gold.aligned_hourly  ← metric embeddings  │
                              │  gold.metric_embeddings (pgvector)          │
                              │  gold.event_embeddings (pgvector, 384D)    │
                              │  gold.predictions (results)                 │
                              │  gold.learning_episodes (SONA, V1.3)       │
                              └──────────────────┬──────────────────────────┘
                                                 │
                              ┌──────────────────┴──────────────────────────┐
                              │   ndp-intelligence-app (separate binary)     │
                              │   Docker container, 512MB limit             │
                              │                                              │
                              │   IntelligenceService                        │
                              │     MetricEmbedder   (z-score → Vec<f32>)  │
                              │     EventEmbedder    (MiniLM → Vec<f32>)   │
                              │     CompositeEmbedder(combines both)        │
                              │     ruvector-core    (HNSW, in-process)    │
                              │     ruvector-sona    (learning, V1.3+)     │
                              │                                              │
                              │   MiniLM model loaded on demand (~200MB)   │
                              │     ─ only if event streams are configured │
                              │     ─ unloaded when not in active cycle    │
                              │                                              │
                              │   Template cache (in-memory, ~50-200       │
                              │     unique templates, <1MB)                 │
                              └─────────────────────────────────────────────┘
```

Container memory limit increased from 256MB (v1.1) to **512MB** to accommodate MiniLM when event streams are configured. Without event streams, actual usage remains ~120MB.

### The Intelligence Cycle (Revised — Two Paths)

The cycle runs in the intelligence binary but spans two architectural layers:

- **Steps 1-3 (OBSERVE → EMBED → REMEMBER):** Feature engineering. Transforms Gold data into prepared features (embeddings). This code lives in `ndp-lib::gold` and can be invoked independently via `ndp gold populate` / `ndp gold embed`.
- **Steps 4-6 (SEARCH → PREDICT → LEARN):** Intelligence. Consumes prepared features to detect patterns and make judgments. This code lives in `ndp-intelligence`.

The binary orchestrates both layers sequentially, but the library boundary is clean.

```
                        ═══════════════════════════════════
                         FEATURE ENGINEERING (Gold layer)
                        ═══════════════════════════════════

┌─ 1. OBSERVE ──────────────────────────────────────────────────────────────┐
│                                                                            │
│  Gold CAs refresh. New hourly bucket appears in aligned view.             │
│                                                                            │
│  ndp-intelligence-app wakes (PG NOTIFY or timer).                        │
│  Reads latest aligned row (numeric fields).                               │
│  Reads event stream table for the same time bucket (text events).        │
│  Reads extracted text feature tables (if configured for this domain).    │
└────────────────────────────────────────────────────────────────────────────┘
                                    │
                        ┌───────────┴───────────┐
                        ▼                       ▼
┌─ 2a. EMBED METRICS ──────────┐  ┌─ 2b. EMBED EVENTS ──────────────────┐
│                                │  │                                      │
│  Gold aligned row → z-score   │  │  For each text event in bucket:     │
│  normalize → Vec<f32> [~32D]  │  │    Match to template cache?         │
│                                │  │    ├─ Hit: reuse cached embedding   │
│  Includes extracted text       │  │    └─ Miss: MiniLM inference         │
│  features as numeric dims     │  │      → cache template + embedding  │
│  (severity_score, etc.)       │  │                                      │
│                                │  │  Compute centroid of all event      │
│  Same as v1.1 for pure        │  │  embeddings in bucket.              │
│  metric fields.               │  │  PCA reduce to ~16-32D.            │
│                                │  │                                      │
│                                │  │  Quantize if configured (PQ8).     │
└────────────────────────────────┘  └──────────────────────────────────────┘
                        │                       │
                        └───────────┬───────────┘
                                    ▼
┌─ 2c. COMPOSE ────────────────────────────────────────────────────────────┐
│                                                                            │
│  Concatenate: [metric_embedding] + [event_centroid] = composite_embedding│
│  Or: metric-only if no event streams configured for this domain.         │
│  Or: event-only if domain is purely text-based.                          │
│                                                                            │
│  Apply quantization if configured.                                        │
└────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─ 3. REMEMBER ────────────────────────────────────────────────────────────┐
│                                                                            │
│  Store composite embedding in pgvector (durable):                        │
│    INSERT INTO gold.metric_embeddings (bucket, domain, embedding, ...)   │
│                                                                            │
│  Store individual event embeddings (if retention policy allows):         │
│    INSERT INTO gold.event_embeddings (bucket, domain, text, embedding)   │
│    Apply retention tier: hot (all, 24h) → warm (anomalous, 30d) → cold │
│                                                                            │
│  Insert into ruvector-core HNSW (fast search):                           │
│    db.insert(VectorEntry { id, vector: composite, metadata })?;          │
└────────────────────────────────────────────────────────────────────────────┘

                        ═══════════════════════════════════
                              INTELLIGENCE (consumer)
                        ═══════════════════════════════════
                                    │
                                    ▼
┌─ 4. SEARCH ──────────────────────────────────────────────────────────────┐
│                                                                            │
│  K-NN search via ruvector-core (unchanged from v1.1):                    │
│    db.search(&SearchQuery { vector: composite, k: 20, ... })?;           │
│                                                                            │
│  Returns 20 most similar past states (metric + textual context match).   │
│  Latency: <100µs.                                                        │
└────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─ 5. PREDICT (unchanged from v1.1) ──────────────────────────────────────┐
│  Look up what happened NEXT for each neighbor.                           │
│  Aggregate: "In 17/20 similar situations, CO2 exceeded 800 within 1h." │
│  Store prediction in gold.predictions.                                   │
└────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─ 6. LEARN (Phase 2+, unchanged from v1.1) ──────────────────────────────┐
│  After prediction window closes: Was prediction correct?                 │
│  Record SONA trajectory. Adapt micro-LoRA.                              │
└────────────────────────────────────────────────────────────────────────────┘
```

### What Triggers Each Step

| Step | Trigger | Frequency | Latency Budget |
|------|---------|-----------|---------------|
| OBSERVE | Gold CA refresh → PG NOTIFY | Every 15 min | N/A |
| EMBED METRICS | After OBSERVE | Every 15 min | <10ms |
| EMBED EVENTS | After OBSERVE | Every 15 min | <50ms (with template cache; ~500ms cold) |
| COMPOSE | After both embeds | Every 15 min | <1ms |
| REMEMBER | After COMPOSE | Every 15 min | <10ms |
| SEARCH | After REMEMBER | Every 15 min | <1ms |
| PREDICT | After SEARCH | Every 15 min | <50ms |
| LEARN | 1 hour after PREDICT | Delayed | <10ms |

**Total intelligence cycle: <130ms** (with template cache warm). First cycle after restart may take ~2s if MiniLM needs loading and templates need caching.

---

## Embedder Trait Architecture

The core abstraction that makes the pipeline domain-agnostic:

```rust
/// Trait for converting domain data into embeddings
pub trait Embedder: Send + Sync {
    /// Produce an embedding vector from a Gold row
    fn embed(&self, row: &GoldRow) -> Result<Embedding>;

    /// Dimensionality of output vectors
    fn dimensions(&self) -> usize;

    /// Human-readable name for logging/config
    fn name(&self) -> &str;
}

/// Output of any Embedder
pub struct Embedding {
    pub vector: Vec<f32>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub quantized: Option<QuantizedVector>,  // if quantization configured
}

/// Quantized representation for storage efficiency
pub enum QuantizedVector {
    PQ8(Vec<u8>),      // Product quantization, 8-bit
    PQ4(Vec<u8>),      // Product quantization, 4-bit
    Binary(Vec<u8>),   // 1-bit, 32x compression
    Scalar(Vec<i8>),   // Scalar quantization, 8-bit
}
```

### Implementations

```
Embedder (trait)
├── MetricEmbedder          ← z-score normalize numeric fields → Vec<f32>
│                              Configured via: fields[], derived[], temporal[]
│                              Output: ~28-64D depending on config
│                              Dependencies: none (pure math)
│
├── EventEmbedder           ← text → MiniLM → Vec<f32> [384D]
│                              Configured via: model, template_cache, severity_filter
│                              Output: 384D (or PCA-reduced)
│                              Dependencies: MiniLM ONNX model (~200MB)
│
└── CompositeEmbedder       ← combines MetricEmbedder + EventEmbedder outputs
                               Configured via: metric_weight, event_weight, pca_dims
                               Output: metric_dims + pca_dims
                               Dependencies: both sub-embedders
```

### Config-Driven Selection

The stream config determines which Embedder is used:

```yaml
streams:
  purpleair:
    intelligence:
      embedding_type: metric        # → MetricEmbedder
      dimensions: 32
      quantization: none
      retention: forever

  nws-forecast-hourly:
    intelligence:
      embedding_type: event         # → EventEmbedder
      model: all-MiniLM-L6-v2
      dimensions: 384
      quantization: none            # low volume
      retention: forever
      template_cache: true

  # Domain-level config combines streams
  domains:
    indoor_air_quality:
      intelligence:
        embedding_type: composite   # → CompositeEmbedder
        metric_streams: [purpleair, nws-observations]
        event_streams: [nws-forecast-hourly]
        event_pca_dims: 16          # reduce 384D → 16D for composite
        quantization: none
        retention: forever
```

---

## Quantization Strategy

### Per-Stream Configuration

Quantization is optional and configured per-stream. The intelligence library supports multiple quantization backends from ruvector-core:

| Method | Compression | Recall Impact | Use When |
|--------|------------|--------------|----------|
| `none` (f32) | 1x | None | Low-volume streams (metrics, few events/day) |
| `scalar` (int8) | 4x | <1% loss | Medium-volume, need some compression |
| `pq8` | 4x | ~2-5% loss | High-volume event streams (sysops logs) |
| `pq4` | 8x | ~5-10% loss | Very high volume, recall less critical |
| `binary` | 32x | ~10-20% loss | Extreme volume, coarse similarity sufficient |

### When Quantization Matters

```
Hourly metric embeddings (32D × f32):   128 bytes/vector  → don't bother
Daily forecast texts (384D × f32):       1.5 KB/vector    → don't bother
Sysops logs (384D × f32, 50K/day):      75 MB/day        → use PQ8 → 19 MB/day
Sysops logs (384D × binary, 50K/day):   2.4 MB/day       → if coarse is OK
```

V1.2 implements quantization and validates it with NWS forecast embeddings (even though unnecessary at that volume), so it's proven before sysops needs it.

---

## Tiered Retention

Event embeddings accumulate faster than metric embeddings. Tiered retention manages storage:

```yaml
retention:
  hot: 24h        # All event embeddings — for "have I seen this before?"
  warm: 30d       # Only anomalous/error events — for pattern learning
  cold: forever   # Hourly centroid embeddings only — permanent memory
```

### How It Works

| Tier | What's Stored | Query Use Case | Storage (sysops, 384D PQ8) |
|------|--------------|---------------|---------------------------|
| **Hot** | All individual event embeddings | "Find similar past log lines" | ~19 MB/day, rolling 24h |
| **Warm** | Events exceeding anomaly threshold | "What unusual events preceded this?" | ~1 MB/day, rolling 30d |
| **Cold** | Hourly centroid embeddings + metric embeddings | "Find similar past operational states" | ~13 MB/year |

A background job ages events through tiers:
1. After 24h: Delete normal-severity event embeddings, keep anomalous ones
2. After 30d: Delete remaining individual event embeddings, keep centroids
3. Centroids live forever (they're the "memory" of that time period)

V1.2 validates this lifecycle with NWS forecast embeddings. Air quality volume is tiny, but the machinery is exercised.

---

## Air Quality Text Use Cases (Dogfooding)

### Available Text Data

NDP already ingests or can ingest:

| Source | Text Content | Volume | Value |
|--------|-------------|--------|-------|
| **NWS Observations** | `textDescription`: "Partly Cloudy" | ~24/day per station | Low (short, categorical) |
| **NWS Forecast** | `detailedForecast`: Multi-sentence narrative | ~4-8/day | **High** (predictive) |
| **NWS Alerts** | Advisory/warning text | Irregular, event-driven | **High** (actionable) |
| **AirNow Forecast** | Narrative forecast text | 1-2/day | **Medium** (domain-specific) |

### The NWS Forecast Use Case

NWS forecast discussions contain direct air quality predictions in natural language:

> "A stagnation advisory is in effect through Thursday. Light winds and inversions will trap pollutants near the surface."

> "Gusty southwest winds will transport smoke from the Riverside Fire into the valley Thursday afternoon."

> "A Pacific front will bring rain and improved air quality by Saturday."

**Embedding these forecasts alongside sensor metrics enables:**

1. **Forecast-aware similarity search**: "Find past hours where sensor readings looked like now AND the forecast discussion was similar." This distinguishes "high PM2.5 with stagnation advisory" (getting worse) from "high PM2.5 with incoming front" (about to improve).

2. **Forecast validation**: After the forecast period passes, compare predicted outcomes (from embedded forecast) with actual sensor readings. "When forecasts mentioned 'stagnation,' how often did PM2.5 actually worsen?" This is a legitimate, useful intelligence product.

3. **Text → outcome correlation**: The similarity engine discovers that forecast texts containing "stagnation" or "inversion" cluster with hours where PM2.5 rose. This validates the text embedding pipeline without writing domain-specific rules.

### Proving the Pipeline

The NWS forecast text proves:
- EventEmbedder works end-to-end (text → template → MiniLM → vector → HNSW → search)
- Template caching works (NWS forecast templates are repetitive)
- CompositeEmbedder combines metric + text embeddings
- Quantization works (validate PQ8 on forecast embeddings, compare recall vs f32)
- Tiered retention works (even if volume doesn't require it)

If any of these fail, we fix them within the air quality domain rather than discovering problems when sysops arrives.

---

## Updated Version Dependency Chain

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                V2.0: MULTI-DOMAIN INTELLIGENCE                              │
│                                                                              │
│  "Add a new domain via config → predictions within 30 days (no code)"      │
│                                                                              │
│  REQUIRES FROM V1.3:                                                        │
│  • Sysops validated as second domain (EventEmbedder at scale)              │
│  • SONA adapters transfer across domains                                    │
│  • Cross-domain composite embeddings (metrics + events + cross-ref)        │
│  • MCP query interface operational ("what's happening across all domains?")│
│  ENHANCED BY RUVECTOR:                                                      │
│  + Cross-domain embedding transfer (weather affects air AND operations)   │
│  + ReasoningBank carries prediction patterns to new domains                │
│  + EWC++ preserves old domain knowledge when learning new domain           │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│           V1.3: ADAPTIVE PREDICTION + SECOND DOMAIN                        │
│                                                                              │
│  "System predicts, learns, and proves generality on a second domain"      │
│                                                                              │
│  REQUIRES FROM V1.2:                                                        │
│  • Validated MetricEmbedder + EventEmbedder + CompositeEmbedder           │
│  • K-NN predictions operational with composite embeddings                  │
│  • Quantization + tiered retention validated                               │
│  • Config-driven embedding pipeline proven                                  │
│  NEW IN V1.3:                                                               │
│  + SONA: 1 base model + micro-LoRA per relationship (~50MB)               │
│  + EWC++ built-in (no separate implementation)                             │
│  + Sysops domain: Docker metrics + log text as second domain               │
│  + MCP query interface for external agents/UI                              │
│  + Q-Learning advisory for graduated autonomy                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│         V1.2: DOMAIN-AGNOSTIC INTELLIGENCE FOUNDATION                      │
│                                                                              │
│  "System remembers situations (metrics + text), identifies candidates,     │
│   and proves every foundational capability on air quality data"            │
│                                                                              │
│  REQUIRES FROM V1.1:                                                        │
│  • Classified streams ................................................ ✅  │
│  • Time-aligned data ................................................. ✅ │
│  • Consistent feature granularity .................................... ✅ │
│  • Objectives declaring which outcomes matter ........................ ✅ │
│  • State transition events ........................................... ✅ │
│  • Feature registry .................................................. ✅ │
│  NEW IN V1.2:                                                               │
│  • Embedder trait (MetricEmbedder + EventEmbedder + CompositeEmbedder)   │
│  • pgvector extension for durable embedding storage                        │
│  • ruvector-core for in-process HNSW + quantization                       │
│  • MiniLM model for text event embedding (loaded on demand)               │
│  • Template caching for efficient text processing                          │
│  • Quantization (PQ8) validated but optional per stream                    │
│  • Tiered retention (hot/warm/cold) validated                              │
│  • K-NN predictive search + outcome lookup                                 │
│  • Granger causality (similarity-guided, same as v1.1)                    │
│  • NWS forecast text as dogfood event stream                               │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    V1.1: GOLD LAYER FOUNDATION ✅ COMPLETE                  │
│                                                                              │
│  Config-driven Gold DDL generation, continuous aggregates,                  │
│  aligned views, feature registry, events infrastructure,                    │
│  stream classification, objectives schema.                                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## V1.2: Domain-Agnostic Intelligence Foundation

### Vision Statement (Revised)

> The system builds a growing library of situations — what the sensors measured AND what the forecasts said. Every hour, it asks: "Have I seen conditions like this before, with a similar forecast context? What happened next?" When it finds meaningful patterns, it flags them. Separately, statistical testing validates causal relationships. Every foundational capability — metric embedding, text embedding, quantization, retention, composite search — is proven on air quality data before any second domain is added.

### Track A: Similarity Intelligence (Metric Path)

Unchanged from v1.1. Provides immediate value from day 30.

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v12-S01** | pgvector Extension Install | Add pgvector to TimescaleDB container | V1.1 Docker |
| **v12-S02** | MetricEmbedder Implementation | Embedder trait + z-score numeric pipeline | V1.1 aligned view |
| **v12-S03** | HNSW Index Bootstrap | Load from pgvector into ruvector-core on startup | v12-S01, S02 |
| **v12-S04** | K-NN Predictive Search | Search K nearest past states, look up outcomes | v12-S03 |
| **v12-S05** | Prediction Generation | Aggregate neighbor outcomes into predictions | v12-S04 |
| **v12-S06** | Prediction Storage | Store in `gold.predictions` with confidence | v12-S05 |
| **v12-S07** | Outcome Tracking | After prediction window, record actual vs predicted | v12-S06 |
| **v12-S08** | Anomaly Detection | Flag hours where embedding distance from clusters exceeds threshold | v12-S03 |
| **v12-S09** | Fingerprinting Dashboard | Grafana panels: clusters, predictions, anomalies | v12-S06, S08 |

### Track B: Statistical Validation (Granger)

Unchanged from v1.1. Provides causal rigor, guided by Track A.

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v12-G01** | Similarity-Guided Candidates | K-NN results identify which stream pairs to test | v12-S04 |
| **v12-G02** | Granger Causality Scanner | Pairwise Granger on top candidates only | v12-G01 |
| **v12-G03** | Lag Optimizer | Find optimal lag per validated relationship | v12-G02 |
| **v12-G04** | Candidate Registry | Store validated correlations with metadata | v12-G02, G03 |
| **v12-G05** | Candidate Ranker | Rank by strength × relevance to objectives | v12-G04 |
| **v12-G06** | Evidence Accumulator | Track relationship stability over time | v12-G04 |
| **v12-G07** | Causal Dashboard | Grafana panels: validated relationships | v12-G04, G05 |

### Track C: Event Intelligence (NEW — Text Dogfooding)

Validates text embedding pipeline using air quality event streams.

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v12-E01** | EventEmbedder Implementation | Embedder trait + MiniLM ONNX inference | v12-I01 (library) |
| **v12-E02** | Template Cache | In-memory template→embedding cache with TTL | v12-E01 |
| **v12-E03** | NWS Forecast Event Stream | Ingest NWS `detailedForecast` text as event stream | V1.1 NWS source |
| **v12-E04** | Event Embedding Storage | Store per-event embeddings in `gold.event_embeddings` | v12-E01, S01 |
| **v12-E05** | CompositeEmbedder Implementation | Combine metric + event centroid embeddings | v12-S02, E01 |
| **v12-E06** | Quantization Validation | PQ8 quantize forecast embeddings, measure recall vs f32 | v12-E04 |
| **v12-E07** | Tiered Retention Implementation | Hot/warm/cold lifecycle for event embeddings | v12-E04 |
| **v12-E08** | Forecast-Aware Search | K-NN on composite embeddings (metrics + forecast text) | v12-E05, S04 |
| **v12-E09** | Forecast Validation Report | Compare forecast text predictions with actual sensor outcomes | v12-E08, S07 |

### Track D: Infrastructure

| ID | Feature | Description | Notes |
|----|---------|-------------|-------|
| **v12-I01** | ndp-intelligence crate (library) | Houses Embedder trait, all intelligence logic | Workspace member |
| **v12-I02** | ndp-intelligence-app (binary) | Standalone daemon/CLI, own Docker container | Separate from ingestion |
| **v12-I03** | IntelligenceService | Core orchestrator: embed → search → predict → learn | Library entry point |
| **v12-I04** | Embedding Config Schema | JSON/YAML schema for per-stream embedding config | Extends stream config |
| **v12-I05** | Intelligence CLI | `ndp intelligence status/search/run/backfill` | Extends ndp-cli |
| **v12-I06** | PG NOTIFY Integration | Gold CA refresh triggers intelligence wake-up | Zero-polling |
| **v12-I07** | Docker Service | `ndp-intelligence` service, 512MB limit | Independent deploy |
| **v12-I08** | MiniLM ONNX Bundle | all-MiniLM-L6-v2 as ONNX, loaded on demand | Only if event streams configured |

### How the Three Tracks Interact

```
Track A (Metrics)          Track C (Events)           Track B (Statistics)

Sensor data → embed        Forecast text → embed
      │                           │
      ▼                           ▼
  MetricEmbedder             EventEmbedder
      │                           │
      └──────────┬────────────────┘
                 ▼
         CompositeEmbedder
                 │
                 ▼
         K-NN search (ruvector-core)
                 │
         "These 20 past hours had similar
          metrics AND similar forecast text"
                 │
                 ├──────────────────────────> Granger validates top
                 │                             candidates (Track B)
                 ▼
         Predict: "In 15/20 similar past
          situations (with stagnation
          advisory), PM2.5 exceeded 35"
```

### Crate + Binary Architecture

The crate boundary reflects the feature engineering / intelligence seam. Embedding and extraction code lives in `ndp-lib` alongside other Gold generators (CAs, features, aligned views), because that's what it is — feature engineering. Intelligence code lives in `ndp-intelligence` as a pure consumer of prepared features.

```
crates/ndp-lib/src/gold/                # FEATURE ENGINEERING (existing crate, extended)
  generators/
    continuous_aggregate.rs             # Existing — CAs from Silver
    aligned_view.rs                     # Existing — cross-stream JOINs
    text_features.rs                    # NEW — extracted feature table DDL
  registry/
    lag.rs, rolling.rs, trend.rs        # Existing feature generators
  embeddings/                           # NEW — Gold row → Vec<f32>
    mod.rs                              # Embedder trait, dispatch
    metric.rs                           # MetricEmbedder (z-score normalize)
    event.rs                            # EventEmbedder (MiniLM + template cache)
    composite.rs                        # CompositeEmbedder (combines both)
    quantization.rs                     # PQ8, PQ4, binary, scalar
  populator/                            # NEW — Rust-based Gold table writers
    text_extractor.rs                   # Text → extracted feature table
    embedding_writer.rs                 # Vec<f32> → pgvector

crates/ndp-intelligence/                # INTELLIGENCE (pure consumer)
  Cargo.toml
  src/
    lib.rs                              # Public API
    config.rs                           # IntelligenceConfig
    traits.rs                           # IntelligenceProvider trait
    similarity.rs                       # ruvector-core wrapper, K-NN, anomaly
    predictions.rs                      # Outcome lookup, aggregation, confidence
    granger.rs                          # Granger causality (pure Rust, ndarray)
    candidates.rs                       # Candidate registry, ranking, evidence
    retention.rs                        # Tiered hot/warm/cold lifecycle
    storage.rs                          # pgvector read/write, predictions, events

apps/ndp-intelligence-app/              # BINARY (orchestrates both layers)
  Cargo.toml
  src/
    main.rs                             # CLI args, daemon, PG LISTEN, one-shot
                                        # Calls ndp-lib for feature engineering,
                                        # then ndp-intelligence for search/predict

models/                                 # ONNX models (git-lfs or download script)
  all-MiniLM-L6-v2.onnx               # ~80MB ONNX, loaded on demand
```

This means `ndp-cli` can invoke feature engineering independently:
- `ndp gold populate --stream system-logs` — extract text features, write to Gold table
- `ndp gold embed --domain indoor-air-quality` — generate embeddings, write to pgvector
- `ndp intelligence run` — full cycle (feature engineering + search/predict/learn)

### Embedding Config (Updated)

Per-stream configuration drives the embedding pipeline:

```json
{
  "stream_id": "nws-forecast-hourly",
  "source": { "...existing..." },
  "intelligence": {
    "enabled": true,
    "embedding_type": "event",
    "event": {
      "model": "all-MiniLM-L6-v2",
      "text_field": "detailedForecast",
      "template_cache": true,
      "template_similarity_threshold": 0.95,
      "severity_filter": null
    },
    "quantization": "none",
    "retention": {
      "hot": "7 days",
      "warm": "90 days",
      "cold": "forever"
    }
  }
}
```

Domain-level config for composite embeddings:

```json
{
  "domain_id": "indoor_air_quality",
  "intelligence": {
    "enabled": true,
    "embedding_type": "composite",
    "metric_streams": ["purpleair", "nws-observations", "home-assistant-state"],
    "event_streams": ["nws-forecast-hourly"],
    "event_pca_dims": 16,
    "search": {
      "k": 20,
      "min_similarity": 0.7,
      "prediction_horizons": ["1 hour", "4 hours"]
    }
  }
}
```

### pgvector Schema (Updated)

```sql
CREATE EXTENSION IF NOT EXISTS vector;

-- Metric/composite state embeddings (one per hourly bucket per domain)
CREATE TABLE gold.metric_embeddings (
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    embedding       vector,                     -- variable dimension per domain
    dimensions      INTEGER NOT NULL,           -- actual dimension count
    quantization    TEXT DEFAULT 'none',         -- none | pq8 | pq4 | binary | scalar
    metadata        JSONB DEFAULT '{}',
    PRIMARY KEY (bucket, domain_id)
);
SELECT create_hypertable('gold.metric_embeddings', 'bucket');

CREATE INDEX idx_metric_embeddings_hnsw ON gold.metric_embeddings
    USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- Individual event embeddings (per-event, with retention tiers)
CREATE TABLE gold.event_embeddings (
    id              BIGSERIAL,
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    stream_id       TEXT NOT NULL,
    event_text      TEXT,                        -- original text (nullable after warm tier)
    template_hash   BIGINT,                      -- for template cache correlation
    embedding       vector(384),                 -- MiniLM output (fixed dimension)
    is_anomalous    BOOLEAN DEFAULT FALSE,       -- flags for warm tier retention
    severity        TEXT,                         -- if applicable (INFO, WARN, ERROR)
    metadata        JSONB DEFAULT '{}',
    retention_tier  TEXT DEFAULT 'hot',           -- hot | warm | cold
    PRIMARY KEY (id, bucket)
);
SELECT create_hypertable('gold.event_embeddings', 'bucket');

CREATE INDEX idx_event_embeddings_hnsw ON gold.event_embeddings
    USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 100);

-- Predictions (unchanged from v1.1)
CREATE TABLE gold.predictions (
    id              BIGSERIAL,
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    metric          TEXT NOT NULL,
    horizon         INTERVAL NOT NULL,
    predicted_value DOUBLE PRECISION,
    predicted_breach BOOLEAN,
    confidence      DOUBLE PRECISION,
    k_neighbors     INTEGER,
    k_supporting    INTEGER,
    actual_value    DOUBLE PRECISION,
    actual_breach   BOOLEAN,
    correct         BOOLEAN,
    embedding_type  TEXT DEFAULT 'metric',       -- NEW: metric | event | composite
    PRIMARY KEY (id, bucket)
);
SELECT create_hypertable('gold.predictions', 'bucket');

-- Causal candidates (unchanged from v1.1)
CREATE TABLE gold.causal_candidates (
    id              SERIAL PRIMARY KEY,
    source_stream   TEXT NOT NULL,
    source_field    TEXT NOT NULL,
    target_stream   TEXT NOT NULL,
    target_field    TEXT NOT NULL,
    lag_minutes     INTEGER,
    correlation     DOUBLE PRECISION,
    granger_p_value DOUBLE PRECISION,
    direction       TEXT,
    evidence_count  INTEGER DEFAULT 0,
    first_detected  TIMESTAMPTZ DEFAULT NOW(),
    last_confirmed  TIMESTAMPTZ DEFAULT NOW(),
    status          TEXT DEFAULT 'candidate'
);

-- Retention management job (TimescaleDB scheduled action)
-- Ages event embeddings through hot → warm → cold tiers
CREATE OR REPLACE FUNCTION gold.age_event_embeddings()
RETURNS void AS $$
BEGIN
    -- Hot → delete (normal events older than hot retention)
    DELETE FROM gold.event_embeddings
    WHERE retention_tier = 'hot'
      AND NOT is_anomalous
      AND bucket < NOW() - INTERVAL '24 hours';

    -- Hot anomalous → warm
    UPDATE gold.event_embeddings
    SET retention_tier = 'warm', event_text = NULL  -- drop raw text to save space
    WHERE retention_tier = 'hot'
      AND is_anomalous
      AND bucket < NOW() - INTERVAL '24 hours';

    -- Warm → delete (older than warm retention)
    DELETE FROM gold.event_embeddings
    WHERE retention_tier = 'warm'
      AND bucket < NOW() - INTERVAL '30 days';
END;
$$ LANGUAGE plpgsql;
```

### V1.2 Acceptance Criteria (Updated)

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| MetricEmbedder operational | 100% of aligned hours embedded | `SELECT count(*) FROM gold.metric_embeddings` |
| EventEmbedder operational | NWS forecast texts embedded | `SELECT count(*) FROM gold.event_embeddings` |
| CompositeEmbedder operational | Composite search returns results | Integration test |
| K-NN search latency | <1ms p99 (composite) | In-process timing |
| Prediction accuracy (30-day, metric-only) | >60% for 1-hour horizon | Outcome tracking |
| Prediction accuracy (composite vs metric) | Composite >= metric | A/B comparison |
| Anomaly detection | Flags >80% of threshold crossings | Compare with events_unified |
| Granger validation | >3 significant relationships | Candidate registry |
| Template cache hit rate | >90% after warmup | Template cache metrics |
| Quantization validated | PQ8 recall >95% vs f32 | Benchmark on forecast embeddings |
| Tiered retention operational | Hot/warm/cold lifecycle runs | Retention job logs |
| Config-driven addition | New stream via config only | Add outdoor-air-quality, no code |
| Pi resource budget | <300MB intelligence container | `docker stats` |
| Startup time | <60s including MiniLM load | Measured on Pi |

---

## V1.3: Adaptive Prediction + Second Domain

### Vision Statement (Revised)

> The system learns from outcomes, adapts per-relationship via SONA, and proves its generality by adding sysops/observability as a second domain. If the architecture is truly config-driven and domain-agnostic, sysops should require zero intelligence code changes — only new stream configs, a new Gold view, and a new domain config. The EventEmbedder (proven on NWS forecasts) handles Docker logs. The MetricEmbedder (proven on sensors) handles container metrics. CompositeEmbedder combines them.

### Sysops Domain (Second Domain Validation)

| Component | Air Quality (proven in V1.2) | Sysops (V1.3) |
|-----------|----------------------------|----------------|
| Metric streams | PurpleAir, NWS obs, HA state | Docker stats, system metrics |
| Event streams | NWS forecasts, alerts | Docker logs, journal entries |
| Gold aligned view | `indoor_air_quality_aligned_hourly` | `operational_health_aligned_hourly` |
| MetricEmbedder config | ~32D (sensor fields) | ~35-45D (container + system fields) |
| EventEmbedder config | NWS text field | Log message field, severity filter |
| Quantization | none (low volume) | PQ8 (high volume logs) |
| Retention | forever (low volume) | Tiered hot/warm/cold |

**Success criterion:** Adding sysops requires only:
1. New stream configs (Docker metrics, Docker logs, system metrics)
2. New Silver hypertable schemas
3. New Gold aligned view DDL
4. New domain intelligence config
5. **Zero changes to ndp-intelligence crate or binary**

### V1.3 Features

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v13-001** | SONA Integration | SonaEngine with NDP's embedding dimensions | v12-S02, E01 |
| **v13-002** | Trajectory Recording | Prediction episodes as SONA trajectories | v12-S07 |
| **v13-003** | Micro-LoRA Adaptation | Per-relationship embedding transformation | v13-001, 002 |
| **v13-004** | ReasoningBank Patterns | Cluster successful prediction patterns | v13-002 |
| **v13-005** | Causal Validation Engine | PC algorithm on Granger candidates | v12-G04 |
| **v13-006** | Action Framework | Actions with preconditions, effects, safety limits | V1.1 objectives |
| **v13-007** | Q-Learning Advisory | Recommend actions, constrained by user ceiling | v13-006 |
| **v13-008** | MCP Query Interface | External query tools for agents/UI | v12-I01 |
| **v13-009** | Sysops Stream Configs | Docker metrics + logs + system metrics streams | V1.1 source traits |
| **v13-010** | Sysops Gold View | `operational_health_aligned_hourly` | v13-009 |
| **v13-011** | Sysops Domain Config | Intelligence config for sysops domain | v13-010 |
| **v13-012** | Cross-Domain Dashboard | Unified view: air quality + sysops | v13-008 |

### SONA Go/No-Go (Unchanged from v1.1)

2-week benchmark: SONA vs ARIMA vs K-NN on CO2 prediction. Proceed only if SONA >= K-NN.

### MCP Query Interface (v13-008)

The intelligence library exposes query functions callable from MCP, CLI, or API:

| Tool | Description | Returns |
|------|------------|---------|
| `current_situation(domain)` | Current state embedding + nearest neighbors | Situation summary |
| `similar_situations(domain, k)` | K-NN search on current state | Past situations + outcomes |
| `predict(domain, metric, horizon)` | Prediction with confidence | Value, breach, confidence |
| `explain_prediction(prediction_id)` | Why this prediction was made | Neighbors, evidence, reasoning |
| `causal_relationships(domain)` | Validated causal graph | Relationships with strength |
| `search_by_conditions(query)` | Find situations matching criteria | Matching time buckets |
| `record_action(domain, action, context)` | External agent records action taken | Action ID |
| `record_outcome(action_id, outcome)` | External agent records result | Updated Q-values |

This makes the Pi an **environmental/operational oracle** queryable by external agents, dashboards, or human operators.

---

## V2.0: Multi-Domain Intelligence (Revised)

### Vision Statement (Revised)

> A user adds energy meter + solar panel streams via config. The platform discovers that weather patterns driving air quality decisions also predict energy costs. Meanwhile, sysops intelligence notices that Bronze write latency increases when energy data volume spikes. Cross-domain correlations emerge automatically. The Pi monitors its environment, itself, and the interactions between them — all through the same config-driven intelligence pipeline, with no domain-specific code.

### V2.0 Capabilities

| Capability | How |
|-----------|-----|
| New domain via config, predictions within 30 days | Proven by sysops in V1.3 |
| Cross-domain composite embeddings | Weather metrics + air quality metrics + sysops metrics |
| Cross-domain causal discovery | "Does CPU temperature affect sensor read accuracy?" |
| Unified MCP oracle | `current_situation(domain="all")` spans every domain |
| EWC++ cross-domain memory | Adding energy doesn't degrade air quality or sysops predictions |
| Federated learning (future) | Binary vector sync across Pi fleet |

---

## Pi Memory Budget (Updated)

| Component | Container | Memory | Cumulative | % of 16GB |
|-----------|-----------|--------|-----------|-----------|
| air-quality-app | ingestion | 123 MB | — | — |
| timescaledb + pgvector | database | 328 MB (+20) | — | — |
| grafana, etcd, mqtt | services | 195 MB | — | — |
| **Existing NDP** | | **646 MB** | **646 MB** | **4.0%** |
| ndp-intelligence binary | intelligence | +50 MB | 696 MB | 4.3% |
| ruvector-core HNSW (metrics) | intelligence | +2 MB | 698 MB | 4.3% |
| pgvector indices (metrics) | database | +5 MB | 703 MB | 4.3% |
| **V1.2 metric-only** | | **+57 MB** | **703 MB** | **4.3%** |
| MiniLM ONNX model (on demand) | intelligence | +200 MB | 903 MB | 5.6% |
| Event embedding HNSW (24h hot) | intelligence | +20 MB | 923 MB | 5.7% |
| Template cache | intelligence | +1 MB | 924 MB | 5.7% |
| pgvector event indices | database | +10 MB | 934 MB | 5.7% |
| **V1.2 with events** | | **+288 MB** | **934 MB** | **5.7%** |
| ruvector-sona (V1.3) | intelligence | +50 MB | 984 MB | 6.1% |
| SONA adapters | intelligence | +0.2 MB | 984 MB | 6.1% |
| ReasoningBank | intelligence | +10 MB | 994 MB | 6.1% |
| Q-Learning tables | intelligence | +6 MB | 1,000 MB | 6.1% |
| Sysops event volume (PQ8) | intelligence | +30 MB | 1,030 MB | 6.3% |
| **V1.3 full** | | **+384 MB** | **1,030 MB** | **6.3%** |

Intelligence container limit: **512 MB**. At V1.2 with events, intelligence uses ~273 MB. At V1.3, ~370 MB. Comfortable headroom.

**At full V1.3 build-out, NDP uses ~6.3% of Pi's RAM.** The 16GB Pi has massive headroom. MiniLM is the largest single addition (+200MB) but is loaded on demand and only when event streams are configured.

---

## Implementation Phases

### Phase 0: Go/No-Go Gate (1 day)

Unchanged from v1.1. Validate ruvector-core compiles on aarch64.

### Phase 1: Metric Intelligence (4-5 weeks)

Same as v1.1 Phase 1A. Deliver Track A (similarity) + Track D (infrastructure).

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v12-I01, S01, S02 (library, pgvector, MetricEmbedder) | Metric embeddings in pgvector |
| 2 | v12-I02, I06, I07 (binary, PG NOTIFY, Docker) | Standalone intelligence on Pi |
| 3 | v12-S03, S04, S05 (HNSW, K-NN, predictions) | Predictions generated |
| 4 | v12-S06, S07, S08 (storage, outcomes, anomaly) | Predictions tracked; anomalies flagged |
| 5 | v12-S09, I05 (dashboard, CLI) | Dashboard + CLI operational |

### Phase 2: Event Intelligence — Dogfooding (3-4 weeks)

New phase. Deliver Track C (event embedding pipeline).

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v12-E01, E02, I08 (EventEmbedder, template cache, MiniLM) | Text embedding produces vectors |
| 2 | v12-E03, E04 (NWS event stream, event storage) | NWS forecasts embedded and stored |
| 3 | v12-E05, E06, E07 (CompositeEmbedder, quantization, retention) | Composite search works; PQ8 validated; retention runs |
| 4 | v12-E08, E09 (forecast-aware search, validation report) | Composite predictions; forecast validation report |

### Phase 3: Statistical Validation (2-3 weeks, can overlap Phase 2)

Same as v1.1 Phase 2. Deliver Track B (Granger).

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v12-G01, G02 (candidates from K-NN, Granger scanner) | Granger tests on top candidates |
| 2 | v12-G03, G04, G05 (lag optimizer, registry, ranker) | Validated relationships |
| 3 | v12-G06, G07 (evidence accumulator, dashboard) | Dashboard shows causal graph |

### Phase 4: SONA + Second Domain (V1.3, 5-6 weeks)

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v13-001, 002 (SONA init, trajectory recording) | SONA recording from predictions |
| 2 | v13-003, 004 (micro-LoRA, ReasoningBank) | Embedding adaptation; go/no-go benchmark |
| 3 | v13-006, 007 (action framework, Q-Learning) | Actions defined; advisory operational |
| 4 | v13-008 (MCP query interface) | External queries operational |
| 5 | v13-009, 010, 011 (sysops streams, Gold view, domain config) | Sysops domain via config only |
| 6 | v13-012 (cross-domain dashboard), integration | Unified dashboard; zero code changes validated |

---

## Risk Assessment (Updated)

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ruvector-core aarch64 compilation | Medium | Medium | Phase 0 compile gate + dual-track from week 1: pgvector always present, ruvector behind Embedder trait |
| ruvector API instability | High | Medium | Pin version, vendor, abstract behind Embedder trait |
| K-NN predictions too noisy | Medium | Medium | Minimum K with min_similarity; fall back to "no prediction" |
| MiniLM too slow on ARM | Medium | Low | Template caching reduces inference to near-zero after warmup; batch mode fallback |
| MiniLM ONNX doesn't run on aarch64 | Low | Medium | ort crate supports ARM; fallback to SimHash for approximate text embedding |
| Composite embeddings don't improve over metric-only | Medium | Low | A/B test in acceptance criteria; composite is additive, metric path still works |
| NWS forecast text too repetitive for meaningful clustering | Medium | Low | Template dedup handles this; even repetitive text validates the pipeline |
| Quantization recall loss unacceptable | Low | Low | PQ8 typically >95% recall; fall back to f32 (just costs more storage) |
| Sysops domain requires code changes (breaks generality claim) | Medium | High | Dogfooding in V1.2 catches architecture gaps before V1.3 |
| MiniLM 200MB too large for Pi budget | Low | Medium | On-demand loading; unload between cycles if memory pressure |

---

## Decision Log (Updated)

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Similarity + statistical dual path | K-NN predicts from day 30; Granger validates causation (from v1.1) |
| 2 | pgvector durable + ruvector-core compute | Complement, not compete (from v1.1) |
| 3 | Intelligence as standalone binary | Workload isolation; independent deployment (from v1.1) |
| 4 | SONA replaces model tournament (V1.3) | 90% memory reduction; EWC++ free (from v1.1) |
| 5 | ~~Numerical embeddings only~~ → **Metric + event embeddings** | Text event streams (logs, forecasts) are a real data shape the platform must handle. The Embedder trait abstracts both behind the same interface |
| 6 | Phase 0 go/no-go gate | De-risk ruvector aarch64 compilation (from v1.1) |
| 7 | Retain Granger causality | Similarity ≠ causation; both needed (from v1.1) |
| 8 | Config-driven embedding pipeline | Per-stream config determines embedding type, quantization, retention |
| 9 | ruvector in intelligence container only | Memory isolation from ingestion (from v1.1) |
| 10 | PG NOTIFY over timer polling | React to data, not clocks (from v1.1) |
| 11 | **Dogfood text embeddings in V1.2 with NWS forecasts** | Proves EventEmbedder, template cache, quantization, retention before sysops needs them. Failures found in familiar domain, not during domain expansion |
| 12 | **Quantization implemented but optional per-stream** | Low-volume streams use f32 (zero complexity). High-volume streams use PQ8 (4x compression). Config-driven, not architectural |
| 13 | **Tiered retention (hot/warm/cold) per-stream** | Per-event embeddings accumulate fast in event-heavy domains. Hot=24h all events, warm=30d anomalous, cold=forever centroids. Config-driven |
| 14 | **MiniLM ONNX loaded on demand** | 200MB model only in memory when event streams exist. Pure-metric domains pay zero cost. Unload between cycles if needed |
| 15 | **Sysops as V1.3 second domain** | Validates "config-driven, no code changes" claim. If sysops requires intelligence code changes, the architecture has a gap. Better to know at V1.3 than V2.0 |
| 16 | **Container limit 512MB (up from 256MB)** | MiniLM + event HNSW + composite embeddings need headroom. 512MB is <3.2% of Pi's 16GB. Still fully isolated from ingestion |
| 17 | **Feature engineering lives in ndp-lib, not ndp-intelligence** | Embedding, text extraction, and composition are feature engineering — same category as CAs, lag features, and aligned views. Intelligence (search/predict/learn) is a pure consumer. This enables `ndp-cli` to run feature engineering without intelligence, keeps the crate boundary clean, and means dashboard-only observability use cases work without the intelligence binary |
| 18 | **Text feature extraction: Rust extraction → Gold hypertable** | SQL (CAs) can't do text extraction. Embedding-only extraction isn't Grafana-queryable. Hybrid: DDL generator creates the table schema from config, Rust populates it. Results are SQL-queryable, joinable into aligned views, available for Granger testing. Extraction logic is iteratable without DDL changes |
| 19 | **Dual-track ruvector validation from week 1** | Build pgvector SQL path as guaranteed baseline AND wire ruvector-core in parallel behind the Embedder trait. If ruvector fails on Pi, flip one config value. Early integration discovers API/memory/SIMD issues with months of runway to adapt, rather than discovering them at V1.3 start |

---

## Appendix A: Crate Dependencies (Updated)

**Library crate** (`crates/ndp-intelligence/Cargo.toml`):

```toml
[dependencies]
ruvector-core = { version = "2.0.1" }

# Text embedding (ONNX Runtime for MiniLM)
ort = { version = "2", features = ["download-binaries"] }  # ONNX Runtime for ARM
tokenizers = "0.21"                                          # HuggingFace tokenizer

# For V1.3:
# ruvector-sona = { version = "0.1" }
# ruvector-attention = { version = "0.1", features = ["simd"] }

# Standard NDP deps
tokio = { version = "1", features = ["full"] }
tokio-postgres = "0.7"
ndp-types = { path = "../ndp-types" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
ndarray = "0.16"
```

**Binary** (`apps/ndp-intelligence-app/Cargo.toml`):

```toml
[dependencies]
ndp-intelligence = { path = "../../crates/ndp-intelligence" }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

## Appendix B: Embedding Dimensions Reference (Updated)

### Metric Embedding (Air Quality, ~32D)

| Category | Fields | Dims |
|----------|--------|------|
| Temporal | hour_sin, hour_cos, is_weekend | 3 |
| Indoor Air | co2, pm25, temp, humidity, voc | 5 |
| Outdoor Weather | temp, humidity, wind_speed, pressure | 4 |
| Outdoor AQI | pm25, aqi | 2 |
| State Events | window_transitions, door_transitions, window_state | 3 |
| Derived | co2_trend_4h, pm25_trend_4h, co2_std_4h, pm25_std_4h, co2_diff_1h, pm25_diff_1h | 6 |
| Forecast Context | forecast_temp, forecast_precip, forecast_wind | 3 |
| **Total** | | **~26-32** |

### Event Embedding (NWS Forecasts, 384D → 16D PCA)

| Component | Dimension |
|-----------|-----------|
| MiniLM output | 384 |
| PCA reduced for composite | 16 |

### Composite Embedding (Air Quality, ~48D)

| Component | Dimension |
|-----------|-----------|
| Metric embedding | ~32 |
| Event centroid (PCA) | ~16 |
| **Composite total** | **~48** |

### Metric Embedding (Sysops, V1.3, ~40D)

| Category | Fields | Dims |
|----------|--------|------|
| Temporal | hour_sin, hour_cos, is_weekend | 3 |
| Per-container (×5) | cpu_pct, mem_pct_limit, net_rate, io_rate | 20 |
| System | cpu_load_1m, ram_pct, disk_pct, cpu_temp | 4 |
| Derived | memory_headroom, container_count, restart_rate | 3 |
| Events | restart_count, oom_kills, health_failures | 3 |
| Log-derived | error_rate, warn_rate, unique_templates | 3 |
| **Total** | | **~36-40** |

## Appendix C: Research References

| Document | Content |
|----------|---------|
| `product/research/ruvector/00-SYNTHESIS.md` | 5-agent research synthesis |
| `product/research/ruvector/01-architecture-fit.md` | Layer mapping, integration patterns |
| `product/research/ruvector/02-event-intelligence.md` | Event embedding, trigger intelligence |
| `product/research/ruvector/03-edge-feasibility.md` | Memory budgets (revised for 16GB) |
| `product/research/ruvector/04-learning-acceleration.md` | SONA vs EWC++ |
| `product/research/ruvector/05-creative-use-cases.md` | 10 creative applications |
| `product/research/ruvector/06-pi5-compilation-feasibility.md` | ARM compilation analysis |
| `product/features/gold-001/MULTI-MODAL-INPUT-ARCHITECTURE.md` | Text/multi-modal input analysis |

---

*Document built on v1.1 foundation, extended with domain-agnostic framing and multi-modal input architecture.*
*The dogfooding principle: if it doesn't work for air quality, it doesn't work. Fix it here.*
*Every foundational capability is proven before claiming generality.*
