# fe-003 Alignment Report

> **Reviewed**: 2026-02-14
> **Reviewer**: ndp-vision-guardian
> **Artifacts reviewed**:
> - `product/features/fe-003/SCOPE.md`
> - `product/features/fe-003/specification/SPECIFICATION.md`
> - `product/features/fe-003/specification/TASK-DECOMPOSITION.md`
> - `product/features/fe-003/architecture/ARCHITECTURE.md`
> - `product/features/fe-003/pseudocode/PSEUDOCODE.md`
> **Vision criteria**: `product/vision/ALIGNMENT-CRITERIA.md`
> **Parent context**: `product/features/gold-002/ARCHITECTURE.md`, `product/features/gold-002/IMPLEMENTATION-ROADMAP.md`

---

## Summary

| Principle | Status | Notes |
|-----------|--------|-------|
| Edge-Only | PASS | All processing on-device, no cloud deps |
| Config-Driven | PASS | Embedding fields, search params, thresholds all in JSON config |
| Domain-Portable | PASS | Generic traits, domain logic only in config |
| Resource-Constrained | WARN | Memory budgets defined; ndarray declared but unused; jemalloc reference in pseudocode |
| Integration-First | PASS | Extends ndp-lib Gold modules, follows generator pattern |
| Privacy by Architecture | PASS | No telemetry, no phone-home, data local-only |
| Self-Learning | PASS | Embedding + search architecture designed for compounding intelligence |

**Overall**: 6 PASS, 1 WARN, 0 VARIANCE, 0 FAIL

**Technical Constraints**: 7 PASS, 0 FAIL

**Cross-Artifact Consistency**: 5 issues found (3 WARN, 2 informational)

---

## Alignment Principle Checks

### 1. Edge-Only -- PASS

**Evidence**:

- SCOPE.md line 5: "Version target: v1.2.0 (Phase 0 + Phase 1 are pre-release, library-only)"
- SCOPE.md line 75: "Phase 1 does NOT deploy to Pi. Library-only."
- ARCHITECTURE.md section 1.1 explicitly forbids dependencies on cloud services or config-client (etcd is V1.3)
- All processing occurs in-process (ruvector-core HNSW) or in local TimescaleDB (pgvector)
- Phase 0 validates compilation on Pi 5 (aarch64) before any foundation work begins
- No network calls in the embedding pipeline. SPECIFICATION.md FR-P1-03 states: "No database dependencies -- pure transformation"

**Assessment**: Fully aligned. All components run on-device. The dual-backend strategy (pgvector + HNSW) keeps everything local. No cloud dependencies of any kind.

### 2. Config-Driven -- PASS

**Evidence**:

- SPECIFICATION.md section 2.5 defines a complete `intelligence` JSON config block with:
  - `embedding.type`: configurable embedding strategy
  - `embedding.fields.temporal`: configurable temporal encodings
  - `embedding.fields.direct`: configurable metric fields with per-field `null_strategy`
  - `embedding.fields.derived`: configurable derived feature fields
  - `search.k`: configurable neighbor count
  - `search.min_similarity`: configurable similarity threshold
  - `search.prediction_horizons`: configurable prediction horizons
  - `anomaly.distance_threshold_sigma`: configurable anomaly threshold
- SPECIFICATION.md FR-P1-07: intelligence block is `Option<IntelligenceConfig>` with `#[serde(default)]` -- does not break existing configs
- SPECIFICATION.md FR-P1-04: warmup threshold (168) is a constructor parameter, configurable
- RunningStats alpha (0.01) is a constructor parameter per SPECIFICATION.md FR-P1-05

**Hardcoded values reviewed**:

| Value | Location | Configurable? | Assessment |
|-------|----------|---------------|------------|
| warmup_threshold = 168 | SPEC FR-P1-04 | Yes (constructor param) | PASS |
| alpha = 0.01 | SPEC FR-P1-05 | Yes (constructor param) | PASS |
| EPSILON = 1e-10 | SPEC FR-P1-05 | No | PASS (implementation constant, not user-facing) |
| MIN_WARMUP_SAMPLES = 24 | PSEUDOCODE 3.1 | Partial | See note below |

**Note on MIN_WARMUP_SAMPLES**: The pseudocode defines `MIN_WARMUP_SAMPLES = 24` as a constant in RunningStats, but the specification defines `warmup_threshold` as a constructor parameter defaulting to 168 on MetricEmbedder. The architecture resolves this: RunningStats has its own warmup threshold for statistical reliability (24), while MetricEmbedder has a higher one for prediction quality (168). Both are constructor parameters. This is acceptable.

**Assessment**: Fully aligned. All user-facing parameters are config-driven via JSON. The intelligence block is optional and backward-compatible.

### 3. Domain-Portable -- PASS

**Evidence**:

- SPECIFICATION.md FR-P1-03: `GoldRow` uses `BTreeMap<String, Option<f64>>` for fields -- no domain-specific field names in the struct
- ARCHITECTURE.md ADR-001: "Typed struct per domain (e.g., IndoorAirQualityRow): Rejected. Would require a new struct for every domain, defeating the config-driven design."
- `Embedder` trait is generic: `fn embed(&self, row: &GoldRow) -> Result<Embedding>` -- any domain can implement it
- `StorageBackend` trait uses `domain_id: String` -- not tied to indoor-air-quality
- `GraphStore` trait is fully generic: nodes have `node_type: String` and `properties: serde_json::Value`
- DDL tables use `domain_id TEXT` column -- multi-domain by design
- The intelligence config block is per-domain in `domain.json`, not hardcoded to indoor-air-quality
- SCOPE.md non-goals confirm domain-specific extensions (EventEmbedder, financial adapter) are deferred to later phases

**Assessment**: Fully aligned. All traits are generic. Domain-specific configuration lives only in JSON config. The same crate structure supports any future domain adapter.

### 4. Resource-Constrained -- WARN

**Evidence**:

- SPECIFICATION.md NFR-02 defines explicit memory budgets:
  - Binary: < 15 MB
  - HNSW index (10K 32D vectors): < 2 MB
  - Total library memory: < 5 MB
- SPECIFICATION.md FR-P1-03: Uses `Vec<f32>` (not `Vec<f64>`) for embedding vectors -- 50% memory savings
- SPECIFICATION.md FR-P1-03: Uses `BTreeMap` for GoldRow fields -- noted as a deliberate choice for deterministic ordering
- Phase 0 gate (P0-05) explicitly measures memory usage and search latency before committing to ruvector
- ARCHITECTURE.md ADR-003: HNSW memory is bounded -- "~2 MB for 10K 32D vectors"
- SPECIFICATION.md section 7: No banned dependencies (Polars, DuckDB, jemalloc) in the dependency list

**Concerns**:

1. **ndarray dependency declared but unused in Phase 1**: SPECIFICATION.md section 7 adds `ndarray = "0.16"` to workspace dependencies with the note "Phase 3 Granger, but declare now." This is a premature dependency addition. ndarray pulls in BLAS/LAPACK infrastructure that may have ARM64 compilation implications. Adding it now when it is not used until Phase 3 adds unnecessary build time and risk.

2. **Pseudocode jemalloc reference**: PSEUDOCODE.md section 1.5 line 249 mentions "measure via jemalloc/malloc stats if available." This is a comment in pseudocode, not a dependency, but should be noted: jemalloc is a banned dependency (crashes on Pi 5 kernel 6.14+). The measurement code must use glibc introspection (`/proc/self/status` VmRSS, which is already the primary approach), not jemalloc.

3. **ARM64 compilation of ruvector**: The entire Phase 0 gate exists to validate this. The risk is identified, mitigated, and gated. This is well-handled.

**Assessment**: WARN due to premature ndarray dependency. The jemalloc pseudocode comment is informational only (not a real dependency). All other resource constraints are well-addressed.

**Recommendation**: Remove `ndarray = "0.16"` from workspace dependencies until Phase 3 begins. Add it when Phase 3 SCOPE.md is written.

### 5. Integration-First -- PASS

**Evidence**:

- SPECIFICATION.md constraint C-04 explicitly requires following existing patterns:
  - "Parsed structs, not file paths"
  - "ConfigLoader trait for config access"
  - "Generator pattern" (PgVectorSchemaGenerator follows ContinuousAggregateGenerator)
  - "Error types use thiserror derive macro"
  - "Module organization: public types re-exported from mod.rs"
  - "Workspace dependencies use { workspace = true }"
- SPECIFICATION.md constraint C-05 lists exactly which files are modified -- limited to ndp-lib Gold extensions, new crates, CLI, and Docker
- ARCHITECTURE.md section 1.1: dependency graph is strictly layered
- ARCHITECTURE.md ADR-005: PgVectorSchemaGenerator explicitly follows ContinuousAggregateGenerator pattern
- ARCHITECTURE.md section 5.1: PgVectorSchemaGenerator integrates with existing `generate_domain()` code path
- No parallel systems created. EmbeddingWriter bridges ndp-lib embeddings to ndp-intelligence storage through the existing code path
- New crates (ndp-intelligence, ndp-intelligence-app) are justified: intelligence is a distinct concern from data pipeline

**Assessment**: Fully aligned. The architecture deliberately extends existing patterns rather than creating parallel systems. New crates are justified by the separation of concerns (intelligence vs. data pipeline). The generator, config, CLI, and module patterns all follow established conventions.

### 6. Privacy by Architecture -- PASS

**Evidence**:

- No telemetry, analytics, or phone-home in any specification
- No network calls outside of local TimescaleDB connections
- ARCHITECTURE.md section 1.1: forbidden dependencies include config-client (etcd is V1.3), no cloud services
- All data stays in local TimescaleDB and in-process HNSW index
- Embedding vectors contain only numerical representations of sensor data -- no PII
- No API endpoints exposed in Phase 1 (library-only)

**Assessment**: Fully aligned. Data never leaves the device. No telemetry infrastructure exists or is planned.

### 7. Self-Learning -- PASS

**Evidence**:

- SPECIFICATION.md FR-P1-04: RunningStats with exponential decay adapts z-score normalization over time -- the longer the system runs, the more accurate the normalization
- SPECIFICATION.md FR-P1-10: `StorageBackend` includes `record_outcome()` for prediction outcome tracking -- closes the feedback loop
- ARCHITECTURE.md ADR-003: "Compounding intelligence -- longer runtime = better" is achieved by:
  - Growing HNSW index (more historical vectors = better K-NN matches)
  - Exponential decay in RunningStats (adapts to seasonal drift)
  - Outcome tracking in `gold.predictions` (correct/incorrect feedback)
- ARCHITECTURE.md ADR-002: "Sensor characteristics drift over time (seasonal temperature shifts, sensor aging). Exponential decay gives recent observations more weight"
- Graph storage (GraphStore trait) enables relationship learning (Phase 3 Granger causality)
- `gold.reasoning_bank` table prepares for V1.3 SONA continual learning (LoRA adapters + EWC++ Fisher information)

**Assessment**: Fully aligned. The architecture is explicitly designed for compounding intelligence. Phase 1 lays the foundation; Phases 2-5 build the full learning loop. The feedback mechanisms (outcome tracking, exponential decay, growing index) are all present.

---

## Technical Constraints Check

| Constraint | Status | Evidence |
|------------|--------|----------|
| ARM64 compatible | PASS | Phase 0 gate validates aarch64 compilation. SPEC NFR-01 requires all code to compile for `aarch64-unknown-linux-gnu`. |
| No banned deps (DuckDB) | PASS | Not referenced in any artifact. TimescaleDB is the only database. |
| No banned deps (Polars) | PASS | Not referenced in any artifact. Uses arrow-rs patterns for Parquet (existing). |
| No banned deps (jemalloc) | PASS | Not a dependency. Pseudocode comment about jemalloc measurement is informational only. |
| TimescaleDB for Silver/Gold | PASS | All intelligence tables in `gold` schema in TimescaleDB. pgvector is a TimescaleDB extension. |
| Config-driven via JSON | PASS | Full `intelligence` config block in `domain.json`. |
| Docker on Pi deployment | PASS | SPEC FR-P1-09 adds pgvector to TimescaleDB Docker image. Phase 1 is library-only (no new container). |
| Bronze -> Silver -> Gold flow | PASS | Intelligence layer reads from Gold aligned views. No short-circuiting of the data flow. |
| Version target correct | PASS | SCOPE.md targets v1.2.0. ALIGNMENT-CRITERIA.md roadmap shows v1.2 as "Discovery engine." |

---

## Scope Alignment

### Scope Gaps

| Item | SCOPE.md Reference | Status |
|------|-------------------|--------|
| None found | -- | All 18 SCOPE.md deliverables (P0-01 through P0-05, P1-01 through P1-13) have corresponding specification entries |

### Scope Additions

| Item | Spec Reference | Classification | Details |
|------|---------------|----------------|---------|
| ndarray workspace dependency | SPEC section 7 | WARN (premature) | SCOPE.md does not mention ndarray. SPEC adds `ndarray = "0.16"` for "Phase 3 Granger, but declare now." Phase 3 is not in scope for fe-003. Recommendation: remove until Phase 3 SCOPE.md is written. |
| `SimilarityEngine` trait stub | SPEC FR-P1-03 / ARCH section 3.2 | Acceptable | SCOPE.md P1-01 mentions "SimilarityEngine trait (stub, no impl in Phase 1)" in the crate structure. The specification defines the full trait signature. This is acceptable -- the trait is a stub with no implementation, and defining it now ensures Phase 2 agents have a clear interface. |
| `Populator` trait | ARCH section 1.4 | Acceptable | ARCHITECTURE.md defines a `Populator` trait in `populator/mod.rs` that is not explicitly in SCOPE.md. This is a thin abstraction layer for EmbeddingWriter and is acceptable as a design refinement. |
| `config.rs` in ndp-intelligence | ARCH section 1.3 | Acceptable | A `config.rs` in ndp-intelligence that re-exports ndp-lib config types. Not in SCOPE.md but is standard practice for crate organization. |
| `predictions/mod.rs` in ndp-intelligence | TASK-DECOMPOSITION section 2 (P1-01) | Acceptable | TASK-DECOMPOSITION lists `predictions/mod.rs` as part of P1-01 crate skeleton. SCOPE.md P1-01 does not explicitly mention this module. It is an empty module declaration for Phase 2 predictions code. |

### Simplifications

| Item | Details | Assessment |
|------|---------|------------|
| EmbeddingWriter location moved | SCOPE.md says `ndp-lib::gold::populator::embedding_writer`. ARCHITECTURE.md moves it to `ndp-intelligence::populator::embedding_writer` to avoid circular dependency. | Acceptable -- documented in ARCHITECTURE.md section 1.6 with clear rationale. The dependency direction (ndp-intelligence -> ndp-lib) is preserved. |
| `ndp-lib::gold::populator/` not created | ARCHITECTURE.md section 1.6: "The `ndp-lib::gold::populator/` module is NOT created." | Acceptable -- this is a consequence of the EmbeddingWriter move. |

---

## Cross-Artifact Consistency

### Issue 1: RunningStats file name disagreement -- WARN

| Artifact | File Name |
|----------|-----------|
| SPECIFICATION.md FR-P1-05 | `crates/ndp-lib/src/gold/embeddings/stats.rs` |
| ARCHITECTURE.md section 1.4 | `crates/ndp-lib/src/gold/embeddings/running_stats.rs` |
| PSEUDOCODE.md cross-reference table | `ndp-lib::gold::embeddings::stats` |
| TASK-DECOMPOSITION P1-05 | `crates/ndp-lib/src/gold/embeddings/stats.rs` |

The specification and task decomposition use `stats.rs`. The architecture uses `running_stats.rs`. The pseudocode module path uses `stats`. Implementation agents should use `stats.rs` (majority consensus across artifacts, and the shorter name is consistent with Rust conventions for module files).

**Recommendation**: Resolve before implementation. Use `stats.rs` as it has 3-of-4 artifact agreement.

### Issue 2: NullStrategy variant disagreement -- WARN

| Artifact | NullStrategy Variants |
|----------|----------------------|
| SPECIFICATION.md FR-P1-04 | `Zero`, `Mean` (2 variants) |
| PSEUDOCODE.md section 2.1 | `Zero`, `LastKnown`, `Mean` (3 variants) |
| PSEUDOCODE.md section 2.5 | Includes `LastKnown` handling logic |
| ARCHITECTURE.md section 5.2 | `null_strategy: String` (accepts any string) |
| SCOPE.md intelligence config | `"null_strategy": "zero"` or `"mean"` |

The specification defines only `Zero` and `Mean`. The pseudocode adds `LastKnown` with full handling logic (section 2.5) including a `last_known: HashMap<String, f64>` field on MetricEmbedder (section 2.1). SCOPE.md shows only `"zero"` and `"mean"` in the config example.

**Recommendation**: Resolve before implementation. If `LastKnown` is desired, add it to the specification. If not, remove it from the pseudocode. The SCOPE.md config example suggests `Zero` and `Mean` only.

### Issue 3: Predictions table `created_at` column -- WARN

| Artifact | Has `created_at`? |
|----------|-------------------|
| SPECIFICATION.md section 2.4 (gold.predictions DDL) | No |
| ARCHITECTURE.md section 4.2.2 (gold.predictions DDL) | Yes: `created_at TIMESTAMPTZ DEFAULT NOW()` |
| PSEUDOCODE.md section 5.6 (generate_predictions_ddl) | No |

The architecture includes `created_at` in the predictions table DDL, but the specification and pseudocode omit it. The specification's DDL should be authoritative since it is the implementation input.

**Recommendation**: Add `created_at TIMESTAMPTZ DEFAULT NOW()` to the specification's predictions DDL to match the architecture. This is a minor DDL field, not a design decision.

### Issue 4: Pending outcomes query filter -- Informational

| Artifact | Filter condition |
|----------|-----------------|
| SPECIFICATION.md FR-P1-10 SQL | `correct IS NULL AND bucket + horizon::interval < NOW()` |
| ARCHITECTURE.md section 4.2.2 | `WHERE correct IS NULL` (partial index) |
| PSEUDOCODE.md section 6.6 | `actual_value IS NULL AND bucket + horizon <= NOW()` |

The specification filters on `correct IS NULL`, the pseudocode filters on `actual_value IS NULL`. Both conditions are semantically similar (a prediction without an outcome has both `actual_value IS NULL` and `correct IS NULL`), but `actual_value IS NULL` is more precise (a prediction could have `correct IS NULL` if no breach prediction was made, per pseudocode section 6.7). The architecture's partial index uses `correct IS NULL` but pseudocode section 5.6 uses `WHERE actual_value IS NULL`.

**Recommendation**: Use `actual_value IS NULL` as the filter condition (more precise). Update the partial index to match.

### Issue 5: EmbeddingWriter cross-reference in pseudocode -- Informational

PSEUDOCODE.md cross-reference table (section 12) lists EmbeddingWriter as living in `ndp-lib::gold::populator::embedding_writer`, but ARCHITECTURE.md section 1.6 moved it to `ndp-intelligence::populator::embedding_writer`. The pseudocode cross-reference table was not updated after the architecture decision.

**Recommendation**: Update PSEUDOCODE.md cross-reference table to reflect the correct location.

---

## Variances Requiring Approval

### 1. ndarray workspace dependency (WARN)

**What**: SPECIFICATION.md section 7 adds `ndarray = "0.16"` to workspace dependencies for "Phase 3 Granger, but declare now."

**Why it matters**: Principle 4 (Resource-Constrained) -- adding unused dependencies increases build time and introduces potential ARM64 compilation risk. Principle 5 (Integration-First) -- adding Phase 3 dependencies during Phase 1 work violates version discipline.

**Recommendation**: Remove ndarray from Phase 1 scope. Add it when Phase 3 SCOPE.md is written. If the user prefers to declare it now for forward planning, document it as an explicit version discipline exception.

**User decision needed**: Keep or remove ndarray from Phase 1 workspace dependencies?

---

## Recommendations

1. **Resolve cross-artifact inconsistencies before implementation**:
   - RunningStats file name: use `stats.rs` (3-of-4 consensus)
   - NullStrategy variants: decide on 2 (`Zero`, `Mean`) or 3 (`Zero`, `Mean`, `LastKnown`)
   - Predictions DDL: add `created_at` to specification to match architecture
   - Pending outcomes filter: standardize on `actual_value IS NULL`
   - Update pseudocode cross-reference table for EmbeddingWriter location

2. **Remove ndarray from Phase 1 workspace dependencies** (or document as explicit exception).

3. **Ensure Phase 0 measurement code uses `/proc/self/status` VmRSS**, not jemalloc introspection (pseudocode already has this as the primary method).

4. **Generate an IMPLEMENTATION-BRIEF.md** from these reviewed artifacts. The specification, architecture, and pseudocode are thorough and well-aligned. Only the minor inconsistencies above need resolution before implementation agents can begin.

---

## Detailed Findings

### 1. Edge-Only

The fe-003 specification is exemplary in its edge-only design. The dual-backend strategy (pgvector for durability + HNSW for speed) keeps all processing local. The Phase 0 gate is a well-structured risk mitigation that validates ARM64 compatibility before committing to ruvector dependencies. No cloud services, no remote APIs, no data export.

The `StorageBackend` trait connects only to local TimescaleDB via `tokio_postgres::Client`. The `GraphStore` trait uses either in-process ruvector-graph or local SQL. There is no network escape hatch.

### 2. Config-Driven

The intelligence configuration block is comprehensive and well-designed. Every parameter that a user might want to tune is exposed in JSON:
- Which fields to embed (temporal, direct, derived)
- How to handle NULLs per field
- Search parameters (k, min_similarity)
- Prediction horizons
- Anomaly thresholds

The `EmbeddingType` enum (`Metric` in Phase 1, extensible to `Event` and `Composite`) allows future embedding strategies without code changes to the config layer.

### 3. Domain-Portable

The `GoldRow` abstraction with `BTreeMap<String, Option<f64>>` is the key design decision. It means the embedding pipeline works with any domain that produces named numeric fields from a Gold aligned view. The architecture explicitly rejects typed-per-domain structs (ADR-001).

### 4. Resource-Constrained

Memory budgets are defined (15 MB binary, 2 MB HNSW index). The choice of `Vec<f32>` over `Vec<f64>` saves 50% memory on embedding vectors. Phase 0 measures actual memory usage before committing. The only concern is the premature ndarray dependency.

### 5. Integration-First

The specification explicitly references existing patterns (ContinuousAggregateGenerator, ConfigLoader, generator pattern) and requires following them. The `PgVectorSchemaGenerator` integrates into the existing `generate_domain()` code path. The DomainConfig extension uses the same `#[serde(default)]` + `Option` pattern as the existing `events` field.

### 6. Privacy by Architecture

No telemetry, no analytics, no cloud. All data stays local. The embedding vectors are numerical representations with no PII content.

### 7. Self-Learning

The architecture explicitly designs for compounding intelligence:
- RunningStats with exponential decay adapts over time
- Growing HNSW index improves K-NN quality
- Outcome tracking closes the prediction feedback loop
- Graph storage enables relationship learning
- `gold.reasoning_bank` prepares for V1.3 SONA continual learning

Phase 1 lays the foundation; the learning loop activates in Phase 2 when predictions begin.

---

*Alignment report for fe-003 Intelligence Foundation Phase 0 + Phase 1. Produced by ndp-vision-guardian against `product/vision/ALIGNMENT-CRITERIA.md`.*
