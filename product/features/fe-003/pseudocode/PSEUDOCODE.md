# fe-003: Intelligence Foundation — Pseudocode

> **Phase**: SPARC Pseudocode (P)
> **Scope**: `product/features/fe-003/SCOPE.md`
> **Architecture**: `product/features/gold-002/ARCHITECTURE.md`
> **Created**: 2026-02-14

---

## Table of Contents

1. [Phase 0: Go/No-Go Test Suite](#1-phase-0-gono-go-test-suite)
2. [MetricEmbedder Algorithm](#2-metricembedder-algorithm)
3. [RunningStats Algorithm](#3-runningstats-algorithm)
4. [Temporal Encoding](#4-temporal-encoding)
5. [PgVectorSchemaGenerator](#5-pgvectorschemagenerator)
6. [StorageBackend Operations](#6-storagebackend-operations)
7. [GraphStore Operations](#7-graphstore-operations)
8. [EmbeddingWriter Populator](#8-embeddingwriter-populator)
9. [CLI Command Flow](#9-cli-command-flow)
10. [Config Deserialization](#10-config-deserialization)

---

## 1. Phase 0: Go/No-Go Test Suite

### 1.1 Cargo.toml Setup

```
ALGORITHM: Phase0CargoSetup
INPUT: none
OUTPUT: Cargo.toml for ruvector smoke test project

BEGIN
    // Create minimal workspace-independent project
    project_dir <- "/tmp/ruvector-arm-test"
    cargo_init(project_dir)

    // Variant A: Full features (default)
    write Cargo.toml:
        [dependencies]
        ruvector-core = "2.0.1"
        ruvector-graph = "0.1"
        tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

    // Variant B: If SimSIMD fails on ARM, disable SIMD
    //   ruvector-core = { version = "2.0.1",
    //       default-features = false,
    //       features = ["storage", "hnsw", "parallel"] }
END
```

### 1.2 Compilation Test

```
ALGORITHM: Phase0CompilationTest
INPUT: project_dir (path)
OUTPUT: CompilationResult { success: bool, variant: A|B, build_time_secs: f64, errors: Vec<String> }

BEGIN
    // Attempt Variant A: all default features
    start_time <- now()
    result_a <- cargo_build_release(project_dir)
    elapsed_a <- now() - start_time

    IF result_a.success THEN
        RETURN CompilationResult {
            success: true,
            variant: A,
            build_time_secs: elapsed_a,
            errors: []
        }
    END IF

    // Check if failure is SimSIMD-related
    IF result_a.stderr CONTAINS "simsimd" OR result_a.stderr CONTAINS "SimSIMD" THEN
        // Attempt Variant B: disable SIMD, scalar fallback
        modify Cargo.toml to use default-features = false
        start_time <- now()
        result_b <- cargo_build_release(project_dir)
        elapsed_b <- now() - start_time

        IF result_b.success THEN
            RETURN CompilationResult {
                success: true,
                variant: B,
                build_time_secs: elapsed_b,
                errors: result_a.errors  // preserve original errors for report
            }
        END IF
    END IF

    RETURN CompilationResult {
        success: false,
        variant: NONE,
        build_time_secs: elapsed_a,
        errors: result_a.errors + result_b.errors
    }
END
```

### 1.3 ruvector-core Smoke Test

```
ALGORITHM: Phase0VectorSmokeTest
INPUT: none
OUTPUT: SmokeTestResult { insert_ok: bool, search_ok: bool, recall_correct: bool }

CONSTANTS:
    N = 100          // vectors to insert
    D = 32           // dimensions (matches MetricEmbedder output)
    K = 5            // neighbors for search
    SEED = 42        // reproducible random seed

BEGIN
    // Initialize ruvector-core VectorDB
    db <- VectorDB::new(VectorDBConfig {
        dimensions: D,
        storage_path: "/tmp/ruvector-smoke-test",
        metric: Cosine,
    })
    IF db IS Err(e) THEN
        RETURN SmokeTestResult { insert_ok: false, search_ok: false, recall_correct: false }
    END IF

    // Generate N deterministic test vectors
    rng <- SeededRng(SEED)
    vectors <- []
    FOR i IN 0..N DO
        v <- generate_random_unit_vector(rng, D)
        vectors.push((format!("vec_{i}"), v))
    END FOR

    // Insert all vectors
    FOR (id, vec) IN vectors DO
        result <- db.insert(id, vec)
        IF result IS Err(e) THEN
            RETURN SmokeTestResult { insert_ok: false, search_ok: false, recall_correct: false }
        END IF
    END FOR

    // Verify count
    assert db.count() == N

    // Search: use vectors[0] as query, expect vectors[0] as top result
    query <- vectors[0].1
    results <- db.search(query, K)
    IF results IS Err(e) THEN
        RETURN SmokeTestResult { insert_ok: true, search_ok: false, recall_correct: false }
    END IF

    // Validate: top result should be vectors[0] itself (similarity ~1.0)
    top_result <- results[0]
    recall_correct <- top_result.id == "vec_0" AND top_result.similarity > 0.99

    // Validate: all K results returned
    search_ok <- results.len() == K

    RETURN SmokeTestResult {
        insert_ok: true,
        search_ok: search_ok,
        recall_correct: recall_correct,
    }
END
```

### 1.4 ruvector-graph Smoke Test

```
ALGORITHM: Phase0GraphSmokeTest
INPUT: none
OUTPUT: GraphSmokeResult { nodes_ok: bool, edges_ok: bool, traversal_ok: bool }

BEGIN
    // Initialize ruvector-graph
    graph <- RuvectorGraph::new(GraphConfig {
        storage_path: "/tmp/ruvector-graph-smoke",
    })
    IF graph IS Err(e) THEN
        RETURN GraphSmokeResult { nodes_ok: false, edges_ok: false, traversal_ok: false }
    END IF

    // Add nodes
    node_a <- GraphNode { id: "metric:co2", node_type: "metric", properties: {} }
    node_b <- GraphNode { id: "metric:temperature", node_type: "metric", properties: {} }
    node_c <- GraphNode { id: "metric:humidity", node_type: "metric", properties: {} }

    result <- graph.add_node(node_a)
    IF result IS Err THEN RETURN { nodes_ok: false, ... } END IF
    result <- graph.add_node(node_b)
    IF result IS Err THEN RETURN { nodes_ok: false, ... } END IF
    result <- graph.add_node(node_c)
    IF result IS Err THEN RETURN { nodes_ok: false, ... } END IF

    nodes_ok <- graph.node_count(None) == 3

    // Add edges
    edge_ab <- GraphEdge {
        source_id: "metric:co2", target_id: "metric:temperature",
        edge_type: "correlates_with", weight: 0.85, properties: {}
    }
    edge_ac <- GraphEdge {
        source_id: "metric:co2", target_id: "metric:humidity",
        edge_type: "correlates_with", weight: 0.72, properties: {}
    }

    result <- graph.add_edge(edge_ab)
    IF result IS Err THEN RETURN { ..., edges_ok: false, ... } END IF
    result <- graph.add_edge(edge_ac)
    IF result IS Err THEN RETURN { ..., edges_ok: false, ... } END IF

    edges_ok <- graph.edge_count(None) == 2

    // Traverse neighbors of co2
    neighbors <- graph.get_neighbors("metric:co2", Some("correlates_with"))
    traversal_ok <- neighbors.len() == 2
        AND neighbors CONTAINS node with id "metric:temperature"
        AND neighbors CONTAINS node with id "metric:humidity"

    RETURN GraphSmokeResult {
        nodes_ok: nodes_ok,
        edges_ok: edges_ok,
        traversal_ok: traversal_ok,
    }
END
```

### 1.5 Memory Measurement

```
ALGORITHM: Phase0MemoryMeasurement
INPUT: db (VectorDB with N vectors)
OUTPUT: MemoryReport { rss_bytes: usize, heap_estimate_bytes: usize, per_vector_bytes: usize }

BEGIN
    // Measure process RSS before and after
    rss_before <- read_proc_self_status("VmRSS")

    // Insert N vectors to an initialized db
    FOR i IN 0..N DO
        db.insert(format!("vec_{i}"), random_vector(D))
    END FOR

    rss_after <- read_proc_self_status("VmRSS")

    rss_delta <- rss_after - rss_before
    per_vector <- rss_delta / N

    // Also measure via jemalloc/malloc stats if available
    heap_estimate <- rss_delta  // conservative; refine with allocator introspection

    RETURN MemoryReport {
        rss_bytes: rss_after,
        heap_estimate_bytes: heap_estimate,
        per_vector_bytes: per_vector,
    }
END

SUBROUTINE: read_proc_self_status(field)
    // Linux-specific: parse /proc/self/status for VmRSS line
    contents <- read_file("/proc/self/status")
    FOR line IN contents.lines() DO
        IF line STARTS WITH field THEN
            RETURN parse_kb_value(line) * 1024
        END IF
    END FOR
    RETURN 0
END
```

### 1.6 Latency Measurement

```
ALGORITHM: Phase0LatencyMeasurement
INPUT: db (VectorDB with N vectors loaded)
OUTPUT: LatencyReport { insert_p50_us, insert_p99_us, search_p50_us, search_p99_us }

CONSTANTS:
    WARMUP_ITERS = 10
    BENCH_ITERS = 100

BEGIN
    // Warmup phase: discard measurements
    FOR _ IN 0..WARMUP_ITERS DO
        db.search(random_vector(D), K)
    END FOR

    // Search latency
    search_times <- []
    FOR i IN 0..BENCH_ITERS DO
        query <- random_vector(D)
        start <- precise_now()
        _ <- db.search(query, K)
        elapsed <- precise_now() - start
        search_times.push(elapsed)
    END FOR

    // Insert latency (use IDs beyond existing range to avoid collisions)
    insert_times <- []
    FOR i IN 0..BENCH_ITERS DO
        start <- precise_now()
        _ <- db.insert(format!("bench_{i}"), random_vector(D))
        elapsed <- precise_now() - start
        insert_times.push(elapsed)
    END FOR

    search_times.sort()
    insert_times.sort()

    RETURN LatencyReport {
        insert_p50_us: percentile(insert_times, 0.50).as_micros(),
        insert_p99_us: percentile(insert_times, 0.99).as_micros(),
        search_p50_us: percentile(search_times, 0.50).as_micros(),
        search_p99_us: percentile(search_times, 0.99).as_micros(),
    }
END
```

### 1.7 Decision Gate Logic

```
ALGORITHM: Phase0DecisionGate
INPUT: compilation: CompilationResult,
       vector_smoke: SmokeTestResult,
       graph_smoke: GraphSmokeResult,
       memory: MemoryReport,
       latency: LatencyReport
OUTPUT: GateDecision { hnsw_backend: HnswBackend, graph_backend: GraphBackend }

ENUM HnswBackend { RuvectorFull, RuvectorScalar, PgVectorOnly }
ENUM GraphBackend { RuvectorGraph, SqlAdjacency }

BEGIN
    // Determine HNSW backend
    hnsw_backend <- match (compilation.success, compilation.variant) {
        (true, A) => {
            IF vector_smoke.search_ok AND vector_smoke.recall_correct THEN
                RuvectorFull
            ELSE
                // Compiles but produces wrong results: fall back to pgvector
                PgVectorOnly
            END IF
        },
        (true, B) => {
            // SimSIMD failed, scalar fallback works
            IF vector_smoke.search_ok AND vector_smoke.recall_correct THEN
                RuvectorScalar
            ELSE
                PgVectorOnly
            END IF
        },
        (false, _) => PgVectorOnly,
    }

    // Determine Graph backend
    graph_backend <- IF compilation.success AND graph_smoke.nodes_ok
                        AND graph_smoke.edges_ok AND graph_smoke.traversal_ok
                     THEN
                         RuvectorGraph
                     ELSE
                         SqlAdjacency
                     END IF

    // Document constraints
    IF memory.per_vector_bytes > 1024 THEN
        warn("Per-vector memory exceeds 1KB — may impact 256MB container budget")
    END IF
    IF latency.search_p99_us > 1000 THEN
        warn("Search p99 exceeds 1ms target")
    END IF

    RETURN GateDecision {
        hnsw_backend: hnsw_backend,
        graph_backend: graph_backend,
    }
END
```

---

## 2. MetricEmbedder Algorithm

### 2.1 Data Structures

```
STRUCT GoldRow {
    bucket: DateTime<Utc>,
    domain_id: String,
    fields: BTreeMap<String, Option<f64>>,
}

STRUCT Embedding {
    vector: Vec<f32>,
    dimensions: usize,
    metadata: HashMap<String, serde_json::Value>,
}

STRUCT MetricEmbedder {
    fields: Vec<EmbeddingField>,         // ordered list of configured fields
    stats: HashMap<String, RunningStats>, // per-field z-score statistics
    dimensions: usize,                    // total output dimensionality
    last_known: HashMap<String, f64>,     // for NullStrategy::LastKnown
}

STRUCT EmbeddingField {
    name: String,
    source: FieldSource,
    null_strategy: NullStrategy,
}

ENUM FieldSource {
    Direct(String),                  // field name from aligned view
    Temporal(TemporalEncoding),      // computed from bucket timestamp
    Derived(String),                 // feature registry field (trend, rolling)
}

ENUM TemporalEncoding { HourSin, HourCos, IsWeekend }

ENUM NullStrategy { Zero, LastKnown, Mean }
```

### 2.2 Constructor

```
ALGORITHM: MetricEmbedder::from_config
INPUT: config: EmbeddingConfig
OUTPUT: Result<MetricEmbedder>

BEGIN
    fields <- []
    stats <- HashMap::new()
    dim_count <- 0

    // Add temporal fields (always 3 dimensions: hour_sin, hour_cos, is_weekend)
    FOR temporal_name IN config.fields.temporal DO
        encoding <- match temporal_name {
            "hour_sin"   => TemporalEncoding::HourSin,
            "hour_cos"   => TemporalEncoding::HourCos,
            "is_weekend" => TemporalEncoding::IsWeekend,
            _            => RETURN Err(InvalidFieldType {
                                field: temporal_name,
                                reason: "unknown temporal encoding"
                            })
        }
        fields.push(EmbeddingField {
            name: temporal_name,
            source: FieldSource::Temporal(encoding),
            null_strategy: NullStrategy::Zero,   // temporals are never null
        })
        dim_count += 1
    END FOR

    // Add direct metric fields
    FOR direct_field IN config.fields.direct DO
        null_strategy <- parse_null_strategy(direct_field.null_strategy)
        fields.push(EmbeddingField {
            name: direct_field.field.clone(),
            source: FieldSource::Direct(direct_field.field),
            null_strategy: null_strategy,
        })
        stats.insert(direct_field.field, RunningStats::new())
        dim_count += 1
    END FOR

    // Add derived feature fields
    FOR derived_name IN config.fields.derived DO
        fields.push(EmbeddingField {
            name: derived_name.clone(),
            source: FieldSource::Derived(derived_name),
            null_strategy: NullStrategy::Zero,  // derived features default zero on null
        })
        stats.insert(derived_name, RunningStats::new())
        dim_count += 1
    END FOR

    RETURN Ok(MetricEmbedder {
        fields: fields,
        stats: stats,
        dimensions: dim_count,
        last_known: HashMap::new(),
    })
END
```

### 2.3 Embed Algorithm

```
ALGORITHM: MetricEmbedder::embed
INPUT: row: &GoldRow
OUTPUT: Result<Embedding>

IMPLEMENTS: Embedder trait

BEGIN
    vector <- Vec::with_capacity(self.dimensions)
    metadata <- HashMap::new()
    null_count <- 0

    FOR field IN &self.fields DO
        value <- match &field.source {

            // Temporal features: computed from timestamp, never null
            FieldSource::Temporal(encoding) => {
                temporal_value(encoding, &row.bucket)
            },

            // Direct fields: read from GoldRow, may be null
            FieldSource::Direct(field_name) => {
                raw <- row.fields.get(field_name)

                match raw {
                    Some(Some(v)) => {
                        // Valid value: observe it for running stats, then z-score
                        self.stats.get_mut(field_name).observe(*v)
                        self.last_known.insert(field_name.clone(), *v)
                        self.z_score(field_name, *v)
                    },
                    Some(None) | None => {
                        // NULL value: handle per strategy
                        null_count += 1
                        self.handle_null(field_name, &field.null_strategy)
                    }
                }
            },

            // Derived fields: same as direct but from derived columns
            FieldSource::Derived(field_name) => {
                raw <- row.fields.get(field_name)

                match raw {
                    Some(Some(v)) => {
                        self.stats.get_mut(field_name).observe(*v)
                        self.z_score(field_name, *v)
                    },
                    Some(None) | None => {
                        null_count += 1
                        self.handle_null(field_name, &field.null_strategy)
                    }
                }
            },
        }

        vector.push(value as f32)
    END FOR

    // Record null count in metadata for downstream quality tracking
    metadata.insert("null_count", json!(null_count))
    metadata.insert("bucket", json!(row.bucket.to_rfc3339()))
    metadata.insert("domain_id", json!(row.domain_id))

    assert vector.len() == self.dimensions

    RETURN Ok(Embedding {
        vector: vector,
        dimensions: self.dimensions,
        metadata: metadata,
    })
END
```

### 2.4 Z-Score Normalization

```
ALGORITHM: MetricEmbedder::z_score
INPUT: field_name: &str, value: f64
OUTPUT: f64

BEGIN
    stats <- self.stats.get(field_name)

    IF NOT stats.is_warmed_up(MIN_WARMUP_SAMPLES) THEN
        // During warmup: return raw value (not normalized)
        // This is safe because warmup embeddings are not used for predictions
        RETURN value
    END IF

    mean <- stats.mean()
    std <- stats.std()

    // Guard against division by zero (constant-value field)
    IF std < EPSILON THEN
        RETURN 0.0
    END IF

    RETURN (value - mean) / std
END
```

### 2.5 NULL Handling

```
ALGORITHM: MetricEmbedder::handle_null
INPUT: field_name: &str, strategy: &NullStrategy
OUTPUT: f64

BEGIN
    match strategy {
        NullStrategy::Zero => {
            // 0.0 in z-score space means "at the mean" — neutral
            RETURN 0.0
        },

        NullStrategy::LastKnown => {
            match self.last_known.get(field_name) {
                Some(last) => {
                    // Use last known value, z-scored
                    RETURN self.z_score(field_name, *last)
                },
                None => {
                    // No prior value exists yet: fall back to zero
                    RETURN 0.0
                }
            }
        },

        NullStrategy::Mean => {
            stats <- self.stats.get(field_name)
            IF stats.is_warmed_up(MIN_WARMUP_SAMPLES) THEN
                // z_score(mean) = 0.0 by definition, but return raw 0.0
                // to avoid floating-point noise
                RETURN 0.0
            ELSE
                RETURN 0.0
            END IF
        },
    }
END
```

### 2.6 Dimensions and Name (Trait Methods)

```
ALGORITHM: MetricEmbedder::dimensions
OUTPUT: usize

BEGIN
    RETURN self.dimensions
END

ALGORITHM: MetricEmbedder::name
OUTPUT: &str

BEGIN
    RETURN "metric"
END
```

### 2.7 Complexity Analysis

```
ANALYSIS: MetricEmbedder::embed

Time Complexity:
    - Temporal features: O(t) where t = temporal field count (always 3)
    - Direct field processing: O(d) where d = direct field count
    - Derived field processing: O(r) where r = derived field count
    - Each field: O(1) for stats lookup, observe, and z-score
    - Total: O(t + d + r) = O(dimensions)

Space Complexity:
    - Output vector: O(dimensions)
    - RunningStats map: O(d + r) — persistent across calls
    - last_known map: O(d) — persistent across calls
    - Per-call allocation: O(dimensions) for the output vector only
```

---

## 3. RunningStats Algorithm

### 3.1 Data Structure

```
STRUCT RunningStats {
    count: u64,                  // total observations
    mean: f64,                   // exponentially decayed mean
    m2: f64,                     // sum of squared deviations (for variance)
    alpha: f64,                  // decay factor (default 0.01)
}

CONSTANTS:
    DEFAULT_ALPHA = 0.01         // slow decay: recent values weighted slightly more
    MIN_WARMUP_SAMPLES = 24      // minimum observations before stats are reliable
    EPSILON = 1e-10              // guard against zero division
```

### 3.2 Constructor

```
ALGORITHM: RunningStats::new
OUTPUT: RunningStats

BEGIN
    RETURN RunningStats {
        count: 0,
        mean: 0.0,
        m2: 0.0,
        alpha: DEFAULT_ALPHA,
    }
END
```

### 3.3 Observe (Welford's Online Algorithm with Exponential Decay)

```
ALGORITHM: RunningStats::observe
INPUT: value: f64
OUTPUT: () (mutates self)

NOTES:
    Uses a modified Welford's algorithm for numerical stability.
    Standard Welford tracks exact running mean/variance; this variant
    applies exponential decay so recent observations are weighted more.
    During the first MIN_WARMUP_SAMPLES observations, uses equal-weight
    Welford to build a reliable baseline.

BEGIN
    self.count += 1

    IF self.count == 1 THEN
        // First observation: initialize
        self.mean <- value
        self.m2 <- 0.0
        RETURN
    END IF

    IF self.count <= MIN_WARMUP_SAMPLES THEN
        // During warmup: standard Welford (equal weight, exact statistics)
        delta <- value - self.mean
        self.mean <- self.mean + delta / (self.count as f64)
        delta2 <- value - self.mean
        self.m2 <- self.m2 + delta * delta2
    ELSE
        // After warmup: exponential decay Welford
        // This gradually forgets old observations, adapting to distribution shifts
        delta <- value - self.mean
        self.mean <- self.mean + self.alpha * delta
        delta2 <- value - self.mean
        self.m2 <- (1.0 - self.alpha) * (self.m2 + self.alpha * delta * delta2)
    END IF
END
```

### 3.4 Mean

```
ALGORITHM: RunningStats::mean
OUTPUT: f64

BEGIN
    RETURN self.mean
END
```

### 3.5 Standard Deviation

```
ALGORITHM: RunningStats::std
OUTPUT: f64

BEGIN
    IF self.count < 2 THEN
        RETURN 0.0
    END IF

    IF self.count <= MIN_WARMUP_SAMPLES THEN
        // During warmup: exact sample standard deviation
        variance <- self.m2 / (self.count as f64 - 1.0)
    ELSE
        // After warmup: exponentially decayed variance
        // m2 already tracks the decayed variance directly
        variance <- self.m2
    END IF

    IF variance < EPSILON THEN
        RETURN 0.0
    END IF

    RETURN sqrt(variance)
END
```

### 3.6 Warmup Check

```
ALGORITHM: RunningStats::is_warmed_up
INPUT: min_samples: usize
OUTPUT: bool

BEGIN
    RETURN self.count >= min_samples as u64
END
```

### 3.7 Numerical Stability Notes

```
ANALYSIS: RunningStats Numerical Stability

Why Welford's variant:
    - Naive variance (sum of squares minus squared sum) accumulates
      catastrophic cancellation error for large datasets
    - Welford's computes variance incrementally with one pass
    - Our exponential decay variant preserves this stability property:
      delta and delta2 are always small relative to the running mean

Edge cases handled:
    1. First observation: mean = value, m2 = 0 (no division by zero)
    2. Constant-value field: m2 stays 0, std() returns 0.0
    3. count < 2: std() returns 0.0 (cannot compute variance from one sample)
    4. Very large values: delta-based update avoids squaring large numbers
    5. NaN/Infinity: should be filtered BEFORE calling observe()

Property: after warmup, the effective window of observations is
approximately 1/alpha = 100 samples (at alpha=0.01). This means the
statistics adapt to distribution shifts over ~100 hours (4 days)
at hourly granularity.
```

### 3.8 Complexity Analysis

```
ANALYSIS: RunningStats

Time Complexity:
    - observe(): O(1) — constant time, no loops
    - mean(): O(1)
    - std(): O(1)
    - is_warmed_up(): O(1)

Space Complexity:
    - O(1) per RunningStats instance (4 fields)
    - Total for MetricEmbedder: O(F) where F = number of fields
```

---

## 4. Temporal Encoding

### 4.1 Core Algorithm

```
ALGORITHM: temporal_features
INPUT: bucket: &DateTime<Utc>
OUTPUT: Vec<f32>

CONSTANTS:
    TWO_PI = 2.0 * std::f64::consts::PI
    HOURS_PER_DAY = 24.0

BEGIN
    hour <- bucket.hour() as f64

    // Cyclical encoding: sin/cos pair captures that 23:00 and 01:00 are close
    hour_sin <- sin(TWO_PI * hour / HOURS_PER_DAY) as f32
    hour_cos <- cos(TWO_PI * hour / HOURS_PER_DAY) as f32

    // Weekend flag: binary feature
    weekday <- bucket.weekday().num_days_from_monday()
    is_weekend <- IF weekday >= 5 THEN 1.0_f32 ELSE 0.0_f32

    RETURN vec![hour_sin, hour_cos, is_weekend]
END
```

### 4.2 Single Temporal Value

```
ALGORITHM: temporal_value
INPUT: encoding: &TemporalEncoding, bucket: &DateTime<Utc>
OUTPUT: f64

NOTES: Called per-field during embed(). Returns individual temporal component.

BEGIN
    hour <- bucket.hour() as f64

    match encoding {
        TemporalEncoding::HourSin => {
            RETURN sin(TWO_PI * hour / HOURS_PER_DAY)
        },
        TemporalEncoding::HourCos => {
            RETURN cos(TWO_PI * hour / HOURS_PER_DAY)
        },
        TemporalEncoding::IsWeekend => {
            weekday <- bucket.weekday().num_days_from_monday()
            RETURN IF weekday >= 5 THEN 1.0 ELSE 0.0
        },
    }
END
```

### 4.3 Design Rationale

```
NOTES: Temporal Encoding Design

Why sin/cos pair instead of raw hour:
    - Raw hour 0 and hour 23 have distance 23 but are only 1 hour apart
    - sin/cos maps hours to a unit circle: distance(23:00, 01:00) is small
    - Two dimensions needed because sin alone is ambiguous (sin(6) == sin(18))

Why is_weekend is binary:
    - Indoor air quality patterns differ fundamentally on weekdays vs weekends
    - Binary encoding is appropriate because there are only 2 states
    - No z-score normalization needed: 0 and 1 are already bounded

No month/season encoding in Phase 1:
    - Only 1 year of hourly data expected initially
    - Seasonal encoding would need sin/cos with period 365.25 days
    - Can be added as derived field in Phase 2 without breaking embedding schema

Temporal features are NOT z-score normalized:
    - sin/cos outputs are already in [-1, 1]
    - is_weekend is already in [0, 1]
    - These ranges are comparable to z-scored metrics (typically [-3, 3])
```

---

## 5. PgVectorSchemaGenerator

### 5.1 Data Structure

```
STRUCT PgVectorSchemaGenerator {
    domain_id: String,
    graph_backend: GraphBackend,     // from Phase 0 decision
}

ENUM GraphBackend { RuvectorGraph, SqlAdjacency }

NOTES:
    Follows the ContinuousAggregateGenerator pattern from
    crates/ndp-lib/src/gold/generators/continuous_aggregate.rs.
    Takes domain config as input, produces DDL strings as output.
    No database connection required — pure generation.
```

### 5.2 Constructor

```
ALGORITHM: PgVectorSchemaGenerator::from_domain_config
INPUT: config: &DomainConfig, graph_backend: GraphBackend
OUTPUT: Result<PgVectorSchemaGenerator>

BEGIN
    // Validate intelligence is configured
    IF config.intelligence.is_none() THEN
        RETURN Err(GoldDdlError::MissingRequiredField {
            field: "intelligence",
            context: format!("domain '{}'", config.id)
        })
    END IF

    intelligence <- config.intelligence.as_ref().unwrap()

    IF NOT intelligence.enabled THEN
        RETURN Err(GoldDdlError::GoldEtlDisabled {
            stream_id: format!("intelligence for domain '{}'", config.id)
        })
    END IF

    RETURN Ok(PgVectorSchemaGenerator {
        domain_id: config.id.clone(),
        graph_backend: graph_backend,
    })
END
```

### 5.3 Generate (Main Entry Point)

```
ALGORITHM: PgVectorSchemaGenerator::generate
INPUT: &self
OUTPUT: Result<String>

NOTES:
    Mirrors ContinuousAggregateGenerator::generate() pattern:
    - Returns complete DDL as a single String
    - Includes header comments, schema creation, all table DDL
    - Uses IF NOT EXISTS for idempotency

BEGIN
    ddl_parts <- vec![
        "-- Intelligence layer DDL for domain: ".to_string() + &self.domain_id,
        "-- Generated by ndp-gold-ddl (PgVectorSchemaGenerator)",
        "",
        "CREATE SCHEMA IF NOT EXISTS gold;",
        "",
    ]

    // 1. pgvector extension (safe to run multiple times)
    ddl_parts.push(self.generate_extension_ddl())
    ddl_parts.push("")

    // 2. metric_embeddings hypertable
    ddl_parts.push(self.generate_metric_embeddings_ddl())
    ddl_parts.push("")

    // 3. predictions hypertable
    ddl_parts.push(self.generate_predictions_ddl())
    ddl_parts.push("")

    // 4. graph tables (SQL fallback only; if ruvector-graph is used, skip)
    IF self.graph_backend == GraphBackend::SqlAdjacency THEN
        ddl_parts.push(self.generate_graph_nodes_ddl())
        ddl_parts.push("")
        ddl_parts.push(self.generate_graph_edges_ddl())
        ddl_parts.push("")
    ELSE
        ddl_parts.push("-- Graph storage: using ruvector-graph backend (no SQL tables needed)")
        ddl_parts.push("")
    END IF

    // 5. reasoning_bank (V1.3 prep, empty)
    ddl_parts.push(self.generate_reasoning_bank_ddl())
    ddl_parts.push("")

    RETURN Ok(ddl_parts.join("\n"))
END
```

### 5.4 Extension DDL

```
ALGORITHM: PgVectorSchemaGenerator::generate_extension_ddl
OUTPUT: String

BEGIN
    RETURN "-- pgvector extension for vector similarity search\n\
            CREATE EXTENSION IF NOT EXISTS vector;"
END
```

### 5.5 Metric Embeddings Table DDL

```
ALGORITHM: PgVectorSchemaGenerator::generate_metric_embeddings_ddl
OUTPUT: String

BEGIN
    RETURN format!(
        r#"-- Metric embeddings hypertable
-- Stores vector embeddings from Gold aligned view rows
CREATE TABLE IF NOT EXISTS gold.metric_embeddings (
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    embedding       vector,
    dimensions      INTEGER NOT NULL,
    metadata        JSONB DEFAULT '{{}}'::jsonb,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (bucket, domain_id)
);

-- Convert to hypertable for time-based partitioning
SELECT create_hypertable('gold.metric_embeddings', 'bucket',
    if_not_exists => TRUE);

-- Index: domain_id for filtered queries
CREATE INDEX IF NOT EXISTS idx_metric_embeddings_domain
    ON gold.metric_embeddings(domain_id, bucket DESC);"#
    )
END
```

### 5.6 Predictions Table DDL

```
ALGORITHM: PgVectorSchemaGenerator::generate_predictions_ddl
OUTPUT: String

BEGIN
    RETURN format!(
        r#"-- Predictions hypertable
-- Stores K-NN similarity-based predictions and outcome tracking
CREATE TABLE IF NOT EXISTS gold.predictions (
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
    evaluated_at    TIMESTAMPTZ,
    PRIMARY KEY (id, bucket)
);

-- Convert to hypertable for time-based partitioning
SELECT create_hypertable('gold.predictions', 'bucket',
    if_not_exists => TRUE);

-- Index: domain + metric for prediction queries
CREATE INDEX IF NOT EXISTS idx_predictions_domain_metric
    ON gold.predictions(domain_id, metric, bucket DESC);

-- Index: unevaluated predictions for outcome tracking
CREATE INDEX IF NOT EXISTS idx_predictions_pending
    ON gold.predictions(domain_id, bucket)
    WHERE actual_value IS NULL;"#
    )
END
```

### 5.7 Graph Nodes DDL (SQL Fallback)

```
ALGORITHM: PgVectorSchemaGenerator::generate_graph_nodes_ddl
OUTPUT: String

BEGIN
    RETURN format!(
        r#"-- Graph nodes table (SQL adjacency fallback)
-- Used when ruvector-graph is not available on this platform
CREATE TABLE IF NOT EXISTS gold.graph_nodes (
    id              TEXT PRIMARY KEY,
    node_type       TEXT NOT NULL,
    properties      JSONB DEFAULT '{{}}'::jsonb,
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_graph_nodes_type
    ON gold.graph_nodes(node_type);"#
    )
END
```

### 5.8 Graph Edges DDL (SQL Fallback)

```
ALGORITHM: PgVectorSchemaGenerator::generate_graph_edges_ddl
OUTPUT: String

BEGIN
    RETURN format!(
        r#"-- Graph edges table (SQL adjacency fallback)
CREATE TABLE IF NOT EXISTS gold.graph_edges (
    id              SERIAL PRIMARY KEY,
    source_id       TEXT NOT NULL REFERENCES gold.graph_nodes(id),
    target_id       TEXT NOT NULL REFERENCES gold.graph_nodes(id),
    edge_type       TEXT NOT NULL,
    weight          DOUBLE PRECISION DEFAULT 1.0,
    properties      JSONB DEFAULT '{{}}'::jsonb,
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_graph_edges_source
    ON gold.graph_edges(source_id, edge_type);

CREATE INDEX IF NOT EXISTS idx_graph_edges_target
    ON gold.graph_edges(target_id, edge_type);"#
    )
END
```

### 5.9 ReasoningBank DDL (V1.3 Prep)

```
ALGORITHM: PgVectorSchemaGenerator::generate_reasoning_bank_ddl
OUTPUT: String

BEGIN
    RETURN format!(
        r#"-- ReasoningBank table (V1.3 SONA prep — empty in V1.2)
-- Stores LoRA adapters and EWC++ Fisher information for ruvector SONA
CREATE TABLE IF NOT EXISTS gold.reasoning_bank (
    id              SERIAL PRIMARY KEY,
    domain_id       TEXT NOT NULL,
    adapter_name    TEXT NOT NULL,
    adapter_blob    BYTEA,
    ewc_fisher      BYTEA,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    performance     JSONB DEFAULT '{{}}'::jsonb
);"#
    )
END
```

### 5.10 Complexity Analysis

```
ANALYSIS: PgVectorSchemaGenerator

Time Complexity:
    - generate(): O(1) — fixed number of DDL templates, string concatenation
    - All sub-generators: O(1) — no loops, no input-dependent computation

Space Complexity:
    - Output string: O(1) — fixed-size DDL (approximately 2-3 KB)
    - No runtime state beyond domain_id and graph_backend enum
```

---

## 6. StorageBackend Operations

### 6.1 Trait Definition

```
TRAIT StorageBackend: Send + Sync {
    // Embeddings
    fn store_embedding(&self, embedding: &StoredEmbedding) -> Result<()>;
    fn load_embeddings(&self, domain_id: &str, since: Option<DateTime<Utc>>)
        -> Result<Vec<StoredEmbedding>>;

    // Predictions
    fn store_prediction(&self, prediction: &Prediction) -> Result<()>;
    fn get_pending_outcomes(&self, domain_id: &str) -> Result<Vec<Prediction>>;
    fn record_outcome(&self, prediction_id: i64, actual: &ActualOutcome) -> Result<()>;
}

STRUCT StoredEmbedding {
    bucket: DateTime<Utc>,
    domain_id: String,
    embedding: Vec<f32>,
    dimensions: usize,
    metadata: serde_json::Value,
}

STRUCT Prediction {
    id: Option<i64>,            // None before INSERT (serial), Some after
    bucket: DateTime<Utc>,
    domain_id: String,
    metric: String,
    horizon: String,            // interval as string, e.g., "1 hour"
    predicted_value: Option<f64>,
    predicted_breach: Option<bool>,
    confidence: f64,
    k_neighbors: i32,
    k_supporting: i32,
    actual_value: Option<f64>,
    actual_breach: Option<bool>,
    correct: Option<bool>,
    evaluated_at: Option<DateTime<Utc>>,
}

STRUCT ActualOutcome {
    actual_value: f64,
    actual_breach: bool,
}
```

### 6.2 PostgresStorage Implementation

```
STRUCT PostgresStorage {
    pool: tokio_postgres::Client,   // or a connection pool wrapper
}
```

### 6.3 store_embedding

```
ALGORITHM: PostgresStorage::store_embedding
INPUT: embedding: &StoredEmbedding
OUTPUT: Result<()>

BEGIN
    // pgvector expects the vector as a text literal: '[0.1, 0.2, ...]'
    vector_literal <- format_pgvector_literal(&embedding.embedding)

    sql <- "INSERT INTO gold.metric_embeddings
                (bucket, domain_id, embedding, dimensions, metadata)
            VALUES ($1, $2, $3::vector, $4, $5)
            ON CONFLICT (bucket, domain_id) DO UPDATE SET
                embedding = EXCLUDED.embedding,
                dimensions = EXCLUDED.dimensions,
                metadata = EXCLUDED.metadata,
                created_at = NOW()"

    self.pool.execute(sql, &[
        &embedding.bucket,
        &embedding.domain_id,
        &vector_literal,
        &(embedding.dimensions as i32),
        &embedding.metadata,
    ]).await?

    RETURN Ok(())
END

SUBROUTINE: format_pgvector_literal
INPUT: vector: &[f32]
OUTPUT: String

BEGIN
    // pgvector text format: "[0.1,0.2,0.3]"
    parts <- vector.iter().map(|v| v.to_string()).collect::<Vec<_>>()
    RETURN format!("[{}]", parts.join(","))
END
```

### 6.4 load_embeddings

```
ALGORITHM: PostgresStorage::load_embeddings
INPUT: domain_id: &str, since: Option<DateTime<Utc>>
OUTPUT: Result<Vec<StoredEmbedding>>

BEGIN
    IF since IS Some(ts) THEN
        sql <- "SELECT bucket, domain_id, embedding, dimensions, metadata
                FROM gold.metric_embeddings
                WHERE domain_id = $1 AND bucket > $2
                ORDER BY bucket ASC"
        rows <- self.pool.query(sql, &[&domain_id, &ts]).await?
    ELSE
        sql <- "SELECT bucket, domain_id, embedding, dimensions, metadata
                FROM gold.metric_embeddings
                WHERE domain_id = $1
                ORDER BY bucket ASC"
        rows <- self.pool.query(sql, &[&domain_id]).await?
    END IF

    embeddings <- Vec::with_capacity(rows.len())

    FOR row IN rows DO
        // pgvector returns the vector as a custom type
        // tokio-postgres pgvector integration parses it to Vec<f32>
        vector <- parse_pgvector_column(row, "embedding")

        embeddings.push(StoredEmbedding {
            bucket: row.get("bucket"),
            domain_id: row.get("domain_id"),
            embedding: vector,
            dimensions: row.get::<_, i32>("dimensions") as usize,
            metadata: row.get("metadata"),
        })
    END FOR

    RETURN Ok(embeddings)
END
```

### 6.5 store_prediction

```
ALGORITHM: PostgresStorage::store_prediction
INPUT: prediction: &Prediction
OUTPUT: Result<()>

BEGIN
    sql <- "INSERT INTO gold.predictions
                (bucket, domain_id, metric, horizon,
                 predicted_value, predicted_breach, confidence,
                 k_neighbors, k_supporting)
            VALUES ($1, $2, $3, $4::interval, $5, $6, $7, $8, $9)"

    self.pool.execute(sql, &[
        &prediction.bucket,
        &prediction.domain_id,
        &prediction.metric,
        &prediction.horizon,
        &prediction.predicted_value,
        &prediction.predicted_breach,
        &prediction.confidence,
        &prediction.k_neighbors,
        &prediction.k_supporting,
    ]).await?

    RETURN Ok(())
END
```

### 6.6 get_pending_outcomes

```
ALGORITHM: PostgresStorage::get_pending_outcomes
INPUT: domain_id: &str
OUTPUT: Result<Vec<Prediction>>

BEGIN
    // Find predictions whose horizon has elapsed but have no actual value recorded
    sql <- "SELECT id, bucket, domain_id, metric, horizon,
                   predicted_value, predicted_breach, confidence,
                   k_neighbors, k_supporting
            FROM gold.predictions
            WHERE domain_id = $1
              AND actual_value IS NULL
              AND bucket + horizon <= NOW()
            ORDER BY bucket ASC
            LIMIT 100"

    rows <- self.pool.query(sql, &[&domain_id]).await?

    predictions <- Vec::with_capacity(rows.len())
    FOR row IN rows DO
        predictions.push(Prediction {
            id: Some(row.get("id")),
            bucket: row.get("bucket"),
            domain_id: row.get("domain_id"),
            metric: row.get("metric"),
            horizon: row.get("horizon"),
            predicted_value: row.get("predicted_value"),
            predicted_breach: row.get("predicted_breach"),
            confidence: row.get("confidence"),
            k_neighbors: row.get("k_neighbors"),
            k_supporting: row.get("k_supporting"),
            actual_value: None,
            actual_breach: None,
            correct: None,
            evaluated_at: None,
        })
    END FOR

    RETURN Ok(predictions)
END
```

### 6.7 record_outcome

```
ALGORITHM: PostgresStorage::record_outcome
INPUT: prediction_id: i64, actual: &ActualOutcome
OUTPUT: Result<()>

BEGIN
    // Retrieve the prediction to compute correctness
    fetch_sql <- "SELECT predicted_breach FROM gold.predictions
                  WHERE id = $1"
    row <- self.pool.query_one(fetch_sql, &[&prediction_id]).await?
    predicted_breach <- row.get::<_, Option<bool>>("predicted_breach")

    // Determine correctness
    correct <- match predicted_breach {
        Some(pb) => Some(pb == actual.actual_breach),
        None => None,   // if no breach prediction was made, cannot evaluate
    }

    // Update the prediction with actual values
    update_sql <- "UPDATE gold.predictions
                   SET actual_value = $1,
                       actual_breach = $2,
                       correct = $3,
                       evaluated_at = NOW()
                   WHERE id = $4"

    self.pool.execute(update_sql, &[
        &actual.actual_value,
        &actual.actual_breach,
        &correct,
        &prediction_id,
    ]).await?

    RETURN Ok(())
END
```

### 6.8 Complexity Analysis

```
ANALYSIS: PostgresStorage

Time Complexity:
    - store_embedding(): O(1) amortized (single INSERT, PRIMARY KEY index update)
    - load_embeddings(): O(n) where n = rows returned (sequential scan or index scan)
    - store_prediction(): O(1) amortized (single INSERT)
    - get_pending_outcomes(): O(n) where n = pending predictions (uses partial index)
    - record_outcome(): O(1) (two queries by primary key)

Space Complexity:
    - Per call: O(n) for result sets
    - Persistent: managed by PostgreSQL — O(total_rows) on disk

Error Handling:
    - Connection errors: propagated as Result::Err, caller retries with backoff
    - Unique constraint violation on store_embedding: handled by ON CONFLICT (upsert)
    - Foreign key violation: should not occur with correct data flow
```

---

## 7. GraphStore Operations

### 7.1 Trait Definition

```
TRAIT GraphStore: Send + Sync {
    fn add_node(&self, node: &GraphNode) -> Result<()>;
    fn add_edge(&self, edge: &GraphEdge) -> Result<()>;
    fn get_edges(&self, node_id: &str, edge_type: Option<&str>) -> Result<Vec<GraphEdge>>;
    fn get_neighbors(&self, node_id: &str, edge_type: Option<&str>) -> Result<Vec<GraphNode>>;
    fn node_count(&self, node_type: Option<&str>) -> Result<usize>;
    fn edge_count(&self, edge_type: Option<&str>) -> Result<usize>;
}

STRUCT GraphNode {
    id: String,
    node_type: String,
    properties: serde_json::Value,
    created_at: DateTime<Utc>,
}

STRUCT GraphEdge {
    source_id: String,
    target_id: String,
    edge_type: String,
    weight: f64,
    properties: serde_json::Value,
    created_at: DateTime<Utc>,
}
```

### 7.2 SqlGraphStore Implementation

```
STRUCT SqlGraphStore {
    pool: tokio_postgres::Client,
}
```

### 7.3 add_node

```
ALGORITHM: SqlGraphStore::add_node
INPUT: node: &GraphNode
OUTPUT: Result<()>

BEGIN
    sql <- "INSERT INTO gold.graph_nodes (id, node_type, properties, created_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                node_type = EXCLUDED.node_type,
                properties = EXCLUDED.properties"

    self.pool.execute(sql, &[
        &node.id,
        &node.node_type,
        &node.properties,
        &node.created_at,
    ]).await?

    RETURN Ok(())
END
```

### 7.4 add_edge

```
ALGORITHM: SqlGraphStore::add_edge
INPUT: edge: &GraphEdge
OUTPUT: Result<()>

BEGIN
    // Validate both nodes exist
    // The FK constraint handles this, but we provide a better error message
    check_sql <- "SELECT id FROM gold.graph_nodes WHERE id = $1 OR id = $2"
    rows <- self.pool.query(check_sql, &[&edge.source_id, &edge.target_id]).await?

    found_ids <- rows.iter().map(|r| r.get::<_, String>("id")).collect::<HashSet<_>>()

    IF NOT found_ids.contains(&edge.source_id) THEN
        RETURN Err(GoldDdlError::FieldNotFound {
            field: edge.source_id.clone(),
            stream_id: "graph_nodes".to_string(),
            available: found_ids.into_iter().collect(),
        })
    END IF

    IF NOT found_ids.contains(&edge.target_id) THEN
        RETURN Err(GoldDdlError::FieldNotFound {
            field: edge.target_id.clone(),
            stream_id: "graph_nodes".to_string(),
            available: found_ids.into_iter().collect(),
        })
    END IF

    sql <- "INSERT INTO gold.graph_edges
                (source_id, target_id, edge_type, weight, properties, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)"

    self.pool.execute(sql, &[
        &edge.source_id,
        &edge.target_id,
        &edge.edge_type,
        &edge.weight,
        &edge.properties,
        &edge.created_at,
    ]).await?

    RETURN Ok(())
END
```

### 7.5 get_edges

```
ALGORITHM: SqlGraphStore::get_edges
INPUT: node_id: &str, edge_type: Option<&str>
OUTPUT: Result<Vec<GraphEdge>>

BEGIN
    match edge_type {
        Some(et) => {
            sql <- "SELECT source_id, target_id, edge_type, weight, properties, created_at
                    FROM gold.graph_edges
                    WHERE source_id = $1 AND edge_type = $2
                    ORDER BY created_at DESC"
            rows <- self.pool.query(sql, &[&node_id, &et]).await?
        },
        None => {
            sql <- "SELECT source_id, target_id, edge_type, weight, properties, created_at
                    FROM gold.graph_edges
                    WHERE source_id = $1
                    ORDER BY created_at DESC"
            rows <- self.pool.query(sql, &[&node_id]).await?
        },
    }

    edges <- Vec::with_capacity(rows.len())
    FOR row IN rows DO
        edges.push(GraphEdge {
            source_id: row.get("source_id"),
            target_id: row.get("target_id"),
            edge_type: row.get("edge_type"),
            weight: row.get("weight"),
            properties: row.get("properties"),
            created_at: row.get("created_at"),
        })
    END FOR

    RETURN Ok(edges)
END
```

### 7.6 get_neighbors

```
ALGORITHM: SqlGraphStore::get_neighbors
INPUT: node_id: &str, edge_type: Option<&str>
OUTPUT: Result<Vec<GraphNode>>

BEGIN
    match edge_type {
        Some(et) => {
            sql <- "SELECT n.id, n.node_type, n.properties, n.created_at
                    FROM gold.graph_nodes n
                    INNER JOIN gold.graph_edges e ON n.id = e.target_id
                    WHERE e.source_id = $1 AND e.edge_type = $2
                    ORDER BY e.weight DESC"
            rows <- self.pool.query(sql, &[&node_id, &et]).await?
        },
        None => {
            sql <- "SELECT n.id, n.node_type, n.properties, n.created_at
                    FROM gold.graph_nodes n
                    INNER JOIN gold.graph_edges e ON n.id = e.target_id
                    WHERE e.source_id = $1
                    ORDER BY e.weight DESC"
            rows <- self.pool.query(sql, &[&node_id]).await?
        },
    }

    neighbors <- Vec::with_capacity(rows.len())
    FOR row IN rows DO
        neighbors.push(GraphNode {
            id: row.get("id"),
            node_type: row.get("node_type"),
            properties: row.get("properties"),
            created_at: row.get("created_at"),
        })
    END FOR

    RETURN Ok(neighbors)
END
```

### 7.7 node_count

```
ALGORITHM: SqlGraphStore::node_count
INPUT: node_type: Option<&str>
OUTPUT: Result<usize>

BEGIN
    match node_type {
        Some(nt) => {
            sql <- "SELECT COUNT(*) as cnt FROM gold.graph_nodes WHERE node_type = $1"
            row <- self.pool.query_one(sql, &[&nt]).await?
        },
        None => {
            sql <- "SELECT COUNT(*) as cnt FROM gold.graph_nodes"
            row <- self.pool.query_one(sql, &[]).await?
        },
    }

    RETURN Ok(row.get::<_, i64>("cnt") as usize)
END
```

### 7.8 edge_count

```
ALGORITHM: SqlGraphStore::edge_count
INPUT: edge_type: Option<&str>
OUTPUT: Result<usize>

BEGIN
    match edge_type {
        Some(et) => {
            sql <- "SELECT COUNT(*) as cnt FROM gold.graph_edges WHERE edge_type = $1"
            row <- self.pool.query_one(sql, &[&et]).await?
        },
        None => {
            sql <- "SELECT COUNT(*) as cnt FROM gold.graph_edges"
            row <- self.pool.query_one(sql, &[]).await?
        },
    }

    RETURN Ok(row.get::<_, i64>("cnt") as usize)
END
```

### 7.9 Complexity Analysis

```
ANALYSIS: SqlGraphStore

Time Complexity:
    - add_node(): O(1) (single INSERT with index update)
    - add_edge(): O(1) (FK validation + INSERT)
    - get_edges(): O(e) where e = edges from source node (index scan on source_id)
    - get_neighbors(): O(e) (JOIN via index)
    - node_count(): O(n) worst case, O(1) with stats (PG can use pg_class.reltuples)
    - edge_count(): O(n) worst case, O(1) with stats

Space Complexity:
    - Per node: O(1) + properties JSONB size
    - Per edge: O(1) + properties JSONB size
    - Indexes: O(n) for nodes, O(e) for edges

Notes:
    - get_neighbors performs a single-hop traversal (1 JOIN)
    - Multi-hop traversal (Phase 3 Granger chains) would use recursive CTEs
    - At V1.2 scale (<1000 nodes, <5000 edges), all operations are sub-millisecond
```

---

## 8. EmbeddingWriter Populator

### 8.1 Data Structure

```
STRUCT EmbeddingWriter {
    storage: Box<dyn StorageBackend>,
    batch_size: usize,              // max embeddings per batch (default: 10)
    retry_count: usize,             // max retries on failure (default: 2)
}
```

### 8.2 Write Single Embedding

```
ALGORITHM: EmbeddingWriter::write
INPUT: embedding: &Embedding, row: &GoldRow
OUTPUT: Result<()>

NOTES:
    Converts an Embedding (from MetricEmbedder) into a StoredEmbedding
    and persists it via the StorageBackend. This is the populator layer
    between the embedder (pure computation) and storage (I/O).

BEGIN
    stored <- StoredEmbedding {
        bucket: row.bucket,
        domain_id: row.domain_id.clone(),
        embedding: embedding.vector.clone(),
        dimensions: embedding.dimensions,
        metadata: serde_json::to_value(&embedding.metadata)?,
    }

    // Attempt write with retry
    last_error <- None
    FOR attempt IN 0..self.retry_count DO
        match self.storage.store_embedding(&stored) {
            Ok(()) => RETURN Ok(()),
            Err(e) => {
                tracing::warn!(
                    attempt = attempt + 1,
                    max = self.retry_count,
                    bucket = %row.bucket,
                    error = %e,
                    "embedding write failed, retrying"
                );
                last_error <- Some(e)
                // Brief pause before retry
                tokio::time::sleep(Duration::from_millis(100 * (attempt + 1) as u64)).await
            }
        }
    END FOR

    // All retries exhausted
    match last_error {
        Some(e) => {
            tracing::error!(
                bucket = %row.bucket,
                domain_id = %row.domain_id,
                "embedding write failed after {} retries: {}",
                self.retry_count, e
            );
            RETURN Err(e)
        },
        None => unreachable!(),
    }
END
```

### 8.3 Write Batch

```
ALGORITHM: EmbeddingWriter::write_batch
INPUT: embeddings: &[(Embedding, GoldRow)]
OUTPUT: Result<WriteReport>

STRUCT WriteReport {
    written: usize,
    failed: usize,
    errors: Vec<(DateTime<Utc>, String)>,   // (bucket, error message)
}

NOTES:
    Processes embeddings in batches. Individual failures do not abort the
    entire batch — partial success is reported. This is important because
    a single NULL-related issue in one row should not prevent other rows
    from being stored.

BEGIN
    report <- WriteReport { written: 0, failed: 0, errors: vec![] }

    FOR chunk IN embeddings.chunks(self.batch_size) DO
        FOR (embedding, row) IN chunk DO
            match self.write(embedding, row).await {
                Ok(()) => {
                    report.written += 1
                },
                Err(e) => {
                    report.failed += 1
                    report.errors.push((row.bucket, e.to_string()))

                    // Log but continue: partial failure is acceptable
                    tracing::warn!(
                        bucket = %row.bucket,
                        error = %e,
                        "skipping embedding for bucket"
                    );
                }
            }
        END FOR
    END FOR

    // Log summary
    tracing::info!(
        written = report.written,
        failed = report.failed,
        "embedding write batch complete"
    );

    RETURN Ok(report)
END
```

### 8.4 Complexity Analysis

```
ANALYSIS: EmbeddingWriter

Time Complexity:
    - write(): O(1) amortized (single DB INSERT + up to retry_count retries)
    - write_batch(): O(n) where n = number of embeddings

Space Complexity:
    - write(): O(dimensions) for the StoredEmbedding copy
    - write_batch(): O(batch_size * dimensions) peak memory

Error Strategy:
    - Transient failures (connection reset): retry with linear backoff
    - Persistent failures (schema mismatch): propagate after retries
    - Batch mode: log and skip individual failures, continue processing
    - Never panic: all errors are Result-based
```

---

## 9. CLI Command Flow

### 9.1 Command Structure

```
COMMAND: ndp gold intelligence schema

USAGE:
    ndp gold intelligence schema [OPTIONS] <DOMAIN>

ARGS:
    <DOMAIN>    Domain target (e.g., "indoor-air-quality")

OPTIONS:
    --config-dir <DIR>     Config directory (default: config/domains/)
    --graph-backend <BE>   Graph backend: ruvector-graph | sql-adjacency (default: sql-adjacency)
    --output <FILE>        Write DDL to file instead of stdout
    --dry-run              Print DDL without executing
```

### 9.2 Main Flow

```
ALGORITHM: CliGoldIntelligenceSchema
INPUT: args: CliArgs
OUTPUT: Result<()>

NOTES:
    This command is added to the existing ndp-cli tool as a new subcommand
    under the existing "gold" entity. Follows the pattern of existing
    "ndp gold sync" and "ndp gold generate" commands.

BEGIN
    // Step 1: Parse arguments
    domain_target <- args.domain
    config_dir <- args.config_dir.unwrap_or("config/domains/".to_string())
    graph_backend <- parse_graph_backend(args.graph_backend.unwrap_or("sql-adjacency"))

    // Step 2: Load domain configuration
    config_path <- format!("{}/{}/domain.json", config_dir, domain_target)

    IF NOT file_exists(&config_path) THEN
        RETURN Err(GoldDdlError::ConfigNotFound { path: config_path })
    END IF

    domain_config <- serde_json::from_str::<DomainConfig>(
        &read_file(&config_path)?
    )?

    // Step 3: Validate intelligence is enabled
    match &domain_config.intelligence {
        None => {
            eprintln!("Domain '{}' has no intelligence configuration.", domain_target);
            eprintln!("Add an 'intelligence' block to {}", config_path);
            RETURN Err(GoldDdlError::MissingRequiredField {
                field: "intelligence",
                context: format!("domain '{}'", domain_target),
            })
        },
        Some(intel) if NOT intel.enabled => {
            eprintln!("Intelligence is disabled for domain '{}'.", domain_target);
            RETURN Err(GoldDdlError::GoldEtlDisabled {
                stream_id: format!("intelligence for '{}'", domain_target),
            })
        },
        Some(_) => {
            // OK: intelligence is present and enabled
        }
    }

    // Step 4: Instantiate generator
    generator <- PgVectorSchemaGenerator::from_domain_config(
        &domain_config,
        graph_backend,
    )?

    // Step 5: Generate DDL
    ddl <- generator.generate()?

    // Step 6: Output
    match args.output {
        Some(output_path) => {
            write_file(&output_path, &ddl)?
            eprintln!("DDL written to {}", output_path);
        },
        None => {
            println!("{}", ddl);
        }
    }

    RETURN Ok(())
END
```

### 9.3 Argument Parsing

```
ALGORITHM: parse_graph_backend
INPUT: s: &str
OUTPUT: Result<GraphBackend>

BEGIN
    match s.to_lowercase().as_str() {
        "ruvector-graph" | "ruvector" => Ok(GraphBackend::RuvectorGraph),
        "sql-adjacency" | "sql" => Ok(GraphBackend::SqlAdjacency),
        _ => Err(format!("Unknown graph backend: '{}'. Use 'ruvector-graph' or 'sql-adjacency'", s))
    }
END
```

### 9.4 Integration with Existing CLI

```
NOTES: CLI Integration

The existing ndp-cli has this structure (from tools/ndp-cli/):

    ndp <entity> <verb> [target]

    Entities: gold, stream, validate, ...
    Gold verbs: generate, sync, ...

New addition:

    ndp gold intelligence schema <domain>

This adds "intelligence" as a sub-entity under "gold", with "schema" as
its verb. The clap derive pattern:

    #[derive(Parser)]
    enum GoldSubcommand {
        Generate { ... },
        Sync { ... },
        Intelligence(IntelligenceSubcommand),  // NEW
    }

    #[derive(Parser)]
    enum IntelligenceSubcommand {
        Schema {
            #[arg()]
            domain: String,
            #[arg(long, default_value = "config/domains/")]
            config_dir: String,
            #[arg(long, default_value = "sql-adjacency")]
            graph_backend: String,
            #[arg(long)]
            output: Option<String>,
        },
    }
```

---

## 10. Config Deserialization

### 10.1 IntelligenceConfig Type Definitions

```
STRUCT IntelligenceConfig {
    enabled: bool,
    embedding: EmbeddingConfig,
    search: SearchConfig,
    anomaly: Option<AnomalyConfig>,
}

STRUCT EmbeddingConfig {
    #[serde(rename = "type")]
    embedding_type: EmbeddingType,
    fields: EmbeddingFieldsConfig,
}

ENUM EmbeddingType {
    Metric,
    // Phase 4: Event, Composite
}

STRUCT EmbeddingFieldsConfig {
    temporal: Vec<String>,               // e.g., ["hour_sin", "hour_cos", "is_weekend"]
    direct: Vec<DirectFieldConfig>,      // metric fields from aligned view
    #[serde(default)]
    derived: Vec<String>,                // feature registry fields (trend, rolling)
}

STRUCT DirectFieldConfig {
    field: String,                       // field name in aligned view
    null_strategy: String,               // "zero", "last_known", or "mean"
}

STRUCT SearchConfig {
    k: usize,
    min_similarity: f64,
    prediction_horizons: Vec<String>,    // e.g., ["1 hour", "4 hours"]
}

STRUCT AnomalyConfig {
    enabled: bool,
    distance_threshold_sigma: f64,       // default: 2.5
}
```

### 10.2 Deserialization Algorithm

```
ALGORITHM: IntelligenceConfig::deserialize
INPUT: json_value: serde_json::Value
OUTPUT: Result<IntelligenceConfig>

NOTES:
    Uses serde derive macros. This pseudocode documents the logical
    deserialization flow, including validation that serde alone cannot express.

BEGIN
    // serde handles structural deserialization automatically via derive
    config <- serde_json::from_value::<IntelligenceConfig>(json_value)?

    // Post-deserialization validation

    // Validate embedding type
    match config.embedding.embedding_type {
        EmbeddingType::Metric => {
            // Valid for Phase 1
        },
        // Future types would be validated here
    }

    // Validate temporal fields are recognized
    valid_temporal <- {"hour_sin", "hour_cos", "is_weekend"}
    FOR name IN &config.embedding.fields.temporal DO
        IF NOT valid_temporal.contains(name) THEN
            RETURN Err(InvalidFeatureConfig {
                feature_type: "temporal",
                message: format!("unknown temporal encoding: '{}'", name),
            })
        END IF
    END FOR

    // Validate null strategies are recognized
    valid_strategies <- {"zero", "last_known", "mean"}
    FOR direct_field IN &config.embedding.fields.direct DO
        IF NOT valid_strategies.contains(&direct_field.null_strategy) THEN
            RETURN Err(InvalidFeatureConfig {
                feature_type: "null_strategy",
                message: format!(
                    "unknown null strategy '{}' for field '{}'. Valid: {:?}",
                    direct_field.null_strategy,
                    direct_field.field,
                    valid_strategies
                ),
            })
        END IF
    END FOR

    // Validate search config bounds
    IF config.search.k == 0 THEN
        RETURN Err(InvalidFeatureConfig {
            feature_type: "search",
            message: "k must be > 0",
        })
    END IF

    IF config.search.min_similarity < 0.0 OR config.search.min_similarity > 1.0 THEN
        RETURN Err(InvalidFeatureConfig {
            feature_type: "search",
            message: "min_similarity must be in [0.0, 1.0]",
        })
    END IF

    // Validate prediction horizons are parseable intervals
    FOR horizon IN &config.search.prediction_horizons DO
        IF NOT is_valid_pg_interval(horizon) THEN
            RETURN Err(InvalidWindow {
                window: horizon.clone(),
            })
        END IF
    END FOR

    RETURN Ok(config)
END
```

### 10.3 DomainConfig Extension (Backward Compatibility)

```
ALGORITHM: DomainConfig extension for intelligence

NOTES:
    The intelligence field is added as Option<IntelligenceConfig> with
    #[serde(default)]. This means:
    - Existing domain.json files without "intelligence" deserialize as None
    - All existing tests pass unchanged
    - New files with "intelligence" gain the full config

STRUCT DomainConfig {
    id: String,
    description: String,
    streams: Vec<StreamRef>,
    alignment: AlignmentConfig,
    #[serde(default)]
    objectives: Vec<ObjectiveConfig>,
    #[serde(default)]
    events: Option<EventsConfig>,
    #[serde(default)]                     // <-- NEW FIELD
    intelligence: Option<IntelligenceConfig>,
}

TEST: backward_compatibility

BEGIN
    // Existing JSON without intelligence block
    json <- r#"{
        "id": "indoor-air-quality",
        "description": "Test domain",
        "streams": [
            { "stream_id": "air-quality", "alias": "indoor", "role": "primary" }
        ],
        "alignment": {
            "view_name": "indoor_air_quality_aligned",
            "granularity": "1 hour"
        }
    }"#

    config <- serde_json::from_str::<DomainConfig>(json)
    assert config.is_ok()
    assert config.unwrap().intelligence.is_none()    // backward compatible
END

TEST: with_intelligence_block

BEGIN
    json <- r#"{
        "id": "indoor-air-quality",
        "description": "Test domain",
        "streams": [
            { "stream_id": "air-quality", "alias": "indoor", "role": "primary" }
        ],
        "alignment": {
            "view_name": "indoor_air_quality_aligned",
            "granularity": "1 hour"
        },
        "intelligence": {
            "enabled": true,
            "embedding": {
                "type": "metric",
                "fields": {
                    "temporal": ["hour_sin", "hour_cos", "is_weekend"],
                    "direct": [
                        { "field": "indoor_co2_mean", "null_strategy": "zero" },
                        { "field": "indoor_pm25_mean", "null_strategy": "zero" }
                    ],
                    "derived": ["indoor_co2_mean_trend_4h"]
                }
            },
            "search": {
                "k": 20,
                "min_similarity": 0.7,
                "prediction_horizons": ["1 hour", "4 hours"]
            }
        }
    }"#

    config <- serde_json::from_str::<DomainConfig>(json)
    assert config.is_ok()

    intel <- config.unwrap().intelligence.unwrap()
    assert intel.enabled == true
    assert intel.embedding.embedding_type == EmbeddingType::Metric
    assert intel.embedding.fields.temporal.len() == 3
    assert intel.embedding.fields.direct.len() == 2
    assert intel.embedding.fields.derived.len() == 1
    assert intel.search.k == 20
    assert intel.anomaly.is_none()      // optional, not provided
END
```

### 10.4 Field Name Validation Against Aligned View

```
ALGORITHM: validate_intelligence_fields
INPUT: intel_config: &IntelligenceConfig, domain_config: &DomainConfig,
       available_fields: &[String]
OUTPUT: Result<()>

NOTES:
    This validation can only run when stream configs are loaded
    (to know what fields exist in the aligned view). It is a
    post-deserialization check, not part of serde.

BEGIN
    // Build set of known aligned view fields
    known_fields <- HashSet::from_iter(available_fields.iter().cloned())

    // Validate direct fields exist in aligned view
    FOR direct_field IN &intel_config.embedding.fields.direct DO
        IF NOT known_fields.contains(&direct_field.field) THEN
            RETURN Err(GoldDdlError::FieldNotFound {
                field: direct_field.field.clone(),
                stream_id: format!("domain '{}' aligned view", domain_config.id),
                available: available_fields.to_vec(),
            })
        END IF
    END FOR

    // Validate derived fields exist (from feature registry)
    FOR derived_name IN &intel_config.embedding.fields.derived DO
        // Derived fields follow naming convention: {field}_{feature}_{window}
        // e.g., "indoor_co2_mean_trend_4h"
        // We validate the base field exists
        IF NOT known_fields.iter().any(|f| derived_name.starts_with(f)) THEN
            tracing::warn!(
                derived = derived_name,
                "derived field '{}' base field not found in aligned view — \
                 it may be generated by the feature registry",
                derived_name
            );
            // Warning only, not error: feature registry may generate it
        END IF
    END FOR

    RETURN Ok(())
END
```

### 10.5 parse_null_strategy Helper

```
ALGORITHM: parse_null_strategy
INPUT: s: &str
OUTPUT: Result<NullStrategy>

BEGIN
    match s.to_lowercase().as_str() {
        "zero"       => Ok(NullStrategy::Zero),
        "last_known" => Ok(NullStrategy::LastKnown),
        "mean"       => Ok(NullStrategy::Mean),
        _ => Err(InvalidFeatureConfig {
            feature_type: "null_strategy",
            message: format!("unknown strategy '{}'. Use zero, last_known, or mean", s),
        })
    }
END
```

### 10.6 Complexity Analysis

```
ANALYSIS: Config Deserialization

Time Complexity:
    - JSON parsing: O(n) where n = config file size
    - Post-parse validation: O(f) where f = number of fields configured
    - Field name validation: O(f * a) where a = available aligned view fields

Space Complexity:
    - IntelligenceConfig struct: O(f) for field lists
    - Validation sets: O(a) for available fields HashSet

Backward Compatibility Guarantee:
    - Adding #[serde(default)] to intelligence field means:
      1. Existing JSON without "intelligence" key -> None
      2. Existing JSON with unknown keys -> ignored by serde (default behavior)
      3. No version negotiation needed
    - Same pattern used for events field (already deployed, proven safe)
```

---

## Summary of Dimensions

The MetricEmbedder for the indoor-air-quality domain produces vectors with:

| Category | Fields | Dimensions |
|----------|--------|-----------|
| Temporal | hour_sin, hour_cos, is_weekend | 3 |
| Direct (indoor) | co2_mean, pm25_mean, temp_mean, humidity_mean | 4 |
| Direct (outdoor) | temp_mean, humidity_mean, wind_speed_mean, aqi_pm25_mean | 4 |
| Derived | co2_trend_4h, pm25_trend_4h, co2_std_4h, co2_diff_1h | 4 |
| **Total** | | **15** |

The actual dimension count is config-driven — adding or removing fields in the intelligence config block changes the embedding dimensionality. All algorithms in this document operate on variable-length vectors.

---

## Cross-References

| Pseudocode Section | Implementing Module | Architecture Section |
|--------------------|--------------------|---------------------|
| Phase 0 Tests | `tests/phase0/` (standalone project) | ARCHITECTURE.md 9 |
| MetricEmbedder | `ndp-lib::gold::embeddings::metric` | ARCHITECTURE.md 3 |
| RunningStats | `ndp-lib::gold::embeddings::stats` | ARCHITECTURE.md 8 |
| Temporal Encoding | `ndp-lib::gold::embeddings::temporal` | ARCHITECTURE.md 8 |
| PgVectorSchemaGenerator | `ndp-lib::gold::generators::pgvector_schema` | ARCHITECTURE.md 4 |
| StorageBackend | `ndp-intelligence::storage::postgres` | ARCHITECTURE.md 3 |
| GraphStore | `ndp-intelligence::graph::sql` | ARCHITECTURE.md 3 |
| EmbeddingWriter | `ndp-lib::gold::populator::embedding_writer` | ARCHITECTURE.md 8 |
| CLI Command | `tools/ndp-cli` (gold intelligence subcommand) | SCOPE.md P1-13 |
| Config Types | `ndp-lib::gold::config::domain` | ARCHITECTURE.md 6 |

---

*Pseudocode for fe-003 Phase 0 + Phase 1. All algorithms are language-agnostic but use Rust-style types (Option, Result, match) to align with the target implementation language.*
