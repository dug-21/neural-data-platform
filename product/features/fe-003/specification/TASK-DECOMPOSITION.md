# fe-003: Task Decomposition — Phase 0 + Phase 1

> **Scope**: `product/features/fe-003/SCOPE.md`
> **Architecture**: `product/features/gold-002/ARCHITECTURE.md`
> **Roadmap**: `product/features/gold-002/IMPLEMENTATION-ROADMAP.md`
> **Created**: 2026-02-14

---

## 1. Implementation Waves

### Wave 0: Go/No-Go Gate (Phase 0)

**Purpose**: Prove ruvector-core and ruvector-graph compile and run on aarch64. This wave is a hard gate -- if it fails, Wave 1 task P1-11 pivots to SQL adjacency tables.

**Tasks**: P0-01, P0-02, P0-03, P0-04, P0-05

**Dependencies**: None (first wave)

**Parallelism**: P0-01 through P0-02 are sequential (compile then run). P0-03 and P0-04 can run in parallel once the binary is available. P0-05 runs last (measurement).

**Estimated agent count**: 1

**Required agents**: `ndp-rust-dev` (with Pi 5 access or cross-compilation toolchain)

---

### Wave 1: Foundation Types (no DB dependencies)

**Purpose**: Establish the crate skeleton, core trait definitions, and pure-Rust types that all subsequent waves depend on. Everything in this wave compiles and tests without a database.

**Tasks**: P1-01, P1-02, P1-03, P1-05, P1-06

**Dependencies**: Wave 0 must complete (determines ruvector-graph feature flag for P1-01 Cargo.toml)

**Parallelism**:
- P1-01 and P1-02 are independent of each other (one is a library crate, the other is a binary crate). Can be done by the same agent sequentially or two agents in parallel.
- P1-03, P1-05, P1-06 depend on P1-01 existing (they live in ndp-lib, which P1-01's Cargo.toml references). P1-05 is a dependency of P1-03 (RunningStats is used by MetricEmbedder), but the trait definition in P1-03 does not require P1-05 to compile.
- P1-06 is independent of P1-03 and P1-05 (config types only).
- Recommended: P1-01 + P1-02 first (skeleton), then P1-03 + P1-05 + P1-06 in parallel.

**Estimated agent count**: 2

**Required agents**: `ndp-rust-dev` (x2) or `ndp-rust-dev` + `ndp-architect` (for trait design review)

---

### Wave 2: Config + Generators (depends on Wave 1 types)

**Purpose**: Extend DomainConfig with the `intelligence` field, implement MetricEmbedder (the first Embedder), and build the PgVectorSchemaGenerator for DDL generation.

**Tasks**: P1-04, P1-07, P1-08

**Dependencies**: Wave 1 must complete (P1-04 needs P1-03 Embedder trait + P1-05 RunningStats; P1-07 needs P1-06 EmbeddingConfig types; P1-08 needs P1-06 config types and follows existing generator pattern)

**Parallelism**:
- P1-04 depends on P1-03 (trait) + P1-05 (RunningStats). Standalone otherwise.
- P1-07 depends on P1-06 (IntelligenceConfig types). Touches `DomainConfig` in ndp-lib -- must be done carefully to avoid breaking existing tests.
- P1-08 depends on P1-06 (config types for table naming). Follows ContinuousAggregateGenerator pattern.
- All three are independent of each other and can run in parallel.

**Estimated agent count**: 2-3

**Required agents**: `ndp-rust-dev` (x2-3), `ndp-tester` (for backward compatibility verification on P1-07)

---

### Wave 3: Storage + Graph (depends on Wave 1, needs integration env)

**Purpose**: Build the durable storage layer (pgvector-backed PostgresStorage) and the graph storage backend. These tasks require TimescaleDB running for integration tests.

**Tasks**: P1-09, P1-10, P1-11

**Dependencies**: Wave 1 must complete (P1-10 needs P1-03 Embedding type; P1-11 needs P1-01 crate structure). Wave 0 outcome determines P1-11 backend choice.

**Parallelism**:
- P1-09 (Docker pgvector) is independent of P1-10 and P1-11 -- it modifies Docker infrastructure only.
- P1-10 (StorageBackend + PostgresStorage) depends on P1-09 completing (needs pgvector extension available for integration tests).
- P1-11 (GraphStore) depends on Wave 0 outcome for backend selection. If ruvector-graph compiles (Wave 0 pass), it wraps ruvector-graph. If not, it implements SQL adjacency using `gold.graph_nodes` / `gold.graph_edges` tables from P1-08's DDL generator.
- Recommended: P1-09 first (quick), then P1-10 and P1-11 in parallel.

**Estimated agent count**: 2

**Required agents**: `ndp-rust-dev` (x2), integration environment active

---

### Wave 4: Integration (depends on Wave 2 + Wave 3)

**Purpose**: Wire everything together -- the EmbeddingWriter that connects MetricEmbedder output to PostgresStorage, and the CLI subcommand that generates intelligence DDL.

**Tasks**: P1-12, P1-13

**Dependencies**: Wave 2 must complete (P1-12 needs P1-04 MetricEmbedder; P1-13 needs P1-08 PgVectorSchemaGenerator). Wave 3 must complete (P1-12 needs P1-10 PostgresStorage).

**Parallelism**:
- P1-12 (EmbeddingWriter) and P1-13 (CLI subcommand) are independent of each other.
- P1-12 requires integration environment (write + read round-trip test).
- P1-13 is a pure CLI addition (generates SQL to stdout, no DB required for basic test).

**Estimated agent count**: 2

**Required agents**: `ndp-rust-dev` (x2), integration environment active for P1-12

---

## 2. Per-Task Detail

### Wave 0: Go/No-Go Gate

#### P0-01: Minimal ruvector Compilation Test

| Field | Value |
|-------|-------|
| **Title** | Create minimal Rust project with ruvector-core + ruvector-graph |
| **Files to create** | `/tmp/ruvector-arm-test/Cargo.toml`, `/tmp/ruvector-arm-test/src/main.rs` (temporary, not in repo) |
| **Files to modify** | None |
| **Dependencies** | None |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | `cargo build --release` succeeds. If SimSIMD fails, retry with `default-features = false, features = ["storage", "hnsw", "parallel"]` |
| **Exit criterion** | Binary compiles for aarch64 (native or cross) |
| **Risk level** | Medium -- SimSIMD C compilation on ARM is the primary risk |
| **Estimated complexity** | S |

#### P0-02: Run on Pi 5

| Field | Value |
|-------|-------|
| **Title** | Execute ruvector test binary on Pi 5 |
| **Files to create** | None |
| **Files to modify** | None |
| **Dependencies** | P0-01 |
| **Agent type** | `ndp-rust-dev` (with Pi access) |
| **Test requirements** | Binary executes without crash, prints success message |
| **Exit criterion** | Binary runs on aarch64 without segfault or runtime error |
| **Risk level** | Low (if P0-01 succeeds, this is likely to succeed) |
| **Estimated complexity** | S |

#### P0-03: Smoke Test ruvector-core

| Field | Value |
|-------|-------|
| **Title** | Insert 100 vectors, K-NN search, verify results |
| **Files to create** | Smoke test in `/tmp/ruvector-arm-test/src/main.rs` (or separate test binary) |
| **Files to modify** | None |
| **Dependencies** | P0-02 |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | Insert 100 random 32D vectors. Search for K=5 nearest neighbors. Verify results are correct (euclidean distance ordering). |
| **Exit criterion** | Correct K-NN results returned on aarch64 |
| **Risk level** | Low |
| **Estimated complexity** | S |

#### P0-04: Smoke Test ruvector-graph

| Field | Value |
|-------|-------|
| **Title** | Add graph nodes + edges, traverse neighbors |
| **Files to create** | Graph test in smoke test binary |
| **Files to modify** | None |
| **Dependencies** | P0-02 |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | Create 5 nodes, 4 edges. Traverse 1-hop neighbors from a node. Verify correct neighbor set. |
| **Exit criterion** | Correct traversal results on aarch64 |
| **Risk level** | Low (ruvector-graph is pure Rust) |
| **Estimated complexity** | S |

#### P0-05: Measure Performance

| Field | Value |
|-------|-------|
| **Title** | Measure memory usage, search latency, build time |
| **Files to create** | `product/features/fe-003/reports/phase0-go-no-go.md` |
| **Files to modify** | None |
| **Dependencies** | P0-03, P0-04 |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | Record: build time (cargo build --release), binary size, RSS after 100 vector inserts, p50/p99 search latency for K=20 over 100 vectors. |
| **Exit criterion** | Go/no-go report written with measurements and backend decision (ruvector vs pgvector-only, ruvector-graph vs SQL adjacency) |
| **Risk level** | Low |
| **Estimated complexity** | S |

---

### Wave 1: Foundation Types

#### P1-01: Create `crates/ndp-intelligence` Crate

| Field | Value |
|-------|-------|
| **Title** | Create ndp-intelligence library crate with module structure |
| **Files to create** | `crates/ndp-intelligence/Cargo.toml`, `crates/ndp-intelligence/src/lib.rs`, `crates/ndp-intelligence/src/config.rs`, `crates/ndp-intelligence/src/error.rs`, `crates/ndp-intelligence/src/similarity/mod.rs`, `crates/ndp-intelligence/src/graph/mod.rs`, `crates/ndp-intelligence/src/predictions/mod.rs`, `crates/ndp-intelligence/src/storage/mod.rs` |
| **Files to modify** | `/workspaces/neural-data-platform/Cargo.toml` (add workspace member `"crates/ndp-intelligence"`) |
| **Dependencies** | Wave 0 (determines whether to include `ruvector-graph` as optional dep) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | `cargo build` succeeds for new crate. `cargo test -p ndp-intelligence` passes (empty test suite is fine). Workspace `cargo check` passes. |
| **Exit criterion** | Crate compiles, is a workspace member, has correct module structure per ARCHITECTURE.md Section 2 |
| **Risk level** | Low |
| **Estimated complexity** | S |

#### P1-02: Create `apps/ndp-intelligence-app` Crate

| Field | Value |
|-------|-------|
| **Title** | Create ndp-intelligence-app binary crate with clap CLI |
| **Files to create** | `apps/ndp-intelligence-app/Cargo.toml`, `apps/ndp-intelligence-app/src/main.rs` |
| **Files to modify** | `/workspaces/neural-data-platform/Cargo.toml` (add workspace member `"apps/ndp-intelligence-app"`) |
| **Dependencies** | P1-01 (depends on ndp-intelligence crate existing) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | `cargo build -p ndp-intelligence-app` succeeds. `ndp-intelligence-app --help` outputs usage with subcommands: `daemon`, `one-shot`, `backfill`, `status`. |
| **Exit criterion** | Binary compiles, `--help` works, subcommand stubs print "not implemented yet" (not TODO/unimplemented! -- print message and exit 1) |
| **Risk level** | Low |
| **Estimated complexity** | S |

#### P1-03: Embedder Trait + GoldRow Types

| Field | Value |
|-------|-------|
| **Title** | Define Embedder trait, GoldRow struct, and Embedding struct |
| **Files to create** | `crates/ndp-lib/src/gold/embeddings/mod.rs`, `crates/ndp-lib/src/gold/embeddings/metric.rs` (empty, placeholder module declaration only) |
| **Files to modify** | `crates/ndp-lib/src/gold/mod.rs` (add `pub mod embeddings;` and re-exports) |
| **Dependencies** | P1-01 (crate must exist for downstream consumers, though types live in ndp-lib) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | Unit tests: (1) GoldRow construction with BTreeMap fields, (2) Embedding construction with vector and metadata, (3) Embedder trait is object-safe (can be used as `dyn Embedder`), (4) GoldRow handles Option<f64> fields correctly |
| **Exit criterion** | Types compile, unit tests pass, trait is object-safe |
| **Risk level** | Low |
| **Estimated complexity** | M |

#### P1-05: RunningStats for Z-Score

| Field | Value |
|-------|-------|
| **Title** | Implement exponential decay mean/std tracker |
| **Files to create** | `crates/ndp-lib/src/gold/embeddings/stats.rs` |
| **Files to modify** | `crates/ndp-lib/src/gold/embeddings/mod.rs` (add `pub mod stats;`) |
| **Dependencies** | P1-03 (module structure must exist) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) Mean converges to known value with synthetic data, (2) Std converges to known value, (3) Exponential decay (alpha=0.01) correctly weights recent values, (4) Z-score output for known input matches expected value within epsilon, (5) Handles warmup period (first N observations) correctly |
| **Exit criterion** | Statistical accuracy tests pass with relative error < 1% for 1000+ samples |
| **Risk level** | Low |
| **Estimated complexity** | M |

#### P1-06: EmbeddingConfig Types

| Field | Value |
|-------|-------|
| **Title** | Define IntelligenceConfig, EmbeddingConfig, SearchConfig, and related config structs |
| **Files to create** | `crates/ndp-lib/src/gold/embeddings/config.rs` |
| **Files to modify** | `crates/ndp-lib/src/gold/embeddings/mod.rs` (add `pub mod config;` and re-exports) |
| **Dependencies** | P1-03 (module structure must exist) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) IntelligenceConfig deserializes from JSON matching ARCHITECTURE.md Section 6, (2) EmbeddingConfig with temporal + direct + derived fields deserializes, (3) SearchConfig with k, min_similarity, prediction_horizons deserializes, (4) Missing `intelligence` key deserializes to None (Option), (5) Round-trip serialize/deserialize preserves all fields |
| **Exit criterion** | All config types deserialize correctly from the sample JSON in ARCHITECTURE.md Section 6 |
| **Risk level** | Low |
| **Estimated complexity** | M |

---

### Wave 2: Config + Generators

#### P1-04: MetricEmbedder Implementation

| Field | Value |
|-------|-------|
| **Title** | Implement MetricEmbedder with z-score normalization, temporal encoding, NULL handling |
| **Files to create** | None (implementation goes in `crates/ndp-lib/src/gold/embeddings/metric.rs`, already declared in P1-03) |
| **Files to modify** | `crates/ndp-lib/src/gold/embeddings/metric.rs` |
| **Dependencies** | P1-03 (Embedder trait, GoldRow), P1-05 (RunningStats), P1-06 (EmbeddingConfig for field configuration) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) Temporal encoding: hour=0 produces sin=0, cos=1; hour=6 produces sin=1, cos=0; hour=12 produces sin=0, cos=-1, (2) Z-score normalization: field with mean=100, std=10, value=120 produces z=2.0, (3) NULL handling with NullStrategy::Zero replaces None with 0.0, (4) NULL handling with NullStrategy::Mean replaces None with running mean, (5) Output dimensions match config (temporal_fields + direct_fields + derived_fields), (6) Weekend detection: Saturday/Sunday produce is_weekend=1.0, (7) Known input/output golden test with 5+ rows |
| **Exit criterion** | All unit tests pass with known inputs producing exact expected outputs |
| **Risk level** | Medium (mathematical correctness matters for downstream intelligence quality) |
| **Estimated complexity** | L |

#### P1-07: DomainConfig `intelligence` Extension

| Field | Value |
|-------|-------|
| **Title** | Add optional `intelligence` field to DomainConfig |
| **Files to modify** | `crates/ndp-lib/src/gold/config/domain.rs` (add `pub intelligence: Option<IntelligenceConfig>` field with `#[serde(default)]`), `config/domains/indoor-air-quality/domain.json` (add `intelligence` block) |
| **Dependencies** | P1-06 (IntelligenceConfig type must exist) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) Existing DomainConfig deserialization tests still pass unchanged (regression), (2) DomainConfig WITHOUT `intelligence` key deserializes with intelligence=None, (3) DomainConfig WITH `intelligence` key deserializes correctly, (4) Full domain.json with intelligence block round-trips, (5) `cargo test -p ndp-lib` full suite passes (no regressions), (6) `cargo test -p ndp-validate` passes, (7) `cargo test -p ndp-gold-ddl` passes |
| **Exit criterion** | Zero test regressions across all crates. New field deserializes correctly. Existing configs without `intelligence` continue to work. |
| **Risk level** | Medium (touching shared config type used by multiple crates) |
| **Estimated complexity** | M |

#### P1-08: PgVector Schema DDL Generator

| Field | Value |
|-------|-------|
| **Title** | Create PgVectorSchemaGenerator following ContinuousAggregateGenerator pattern |
| **Files to create** | `crates/ndp-lib/src/gold/generators/pgvector_schema.rs` |
| **Files to modify** | `crates/ndp-lib/src/gold/generators/mod.rs` (add `pub mod pgvector_schema;` and re-export), `crates/ndp-lib/src/gold/mod.rs` (add re-export) |
| **Dependencies** | P1-06 (config types for table naming conventions) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) Generates `CREATE EXTENSION IF NOT EXISTS vector;`, (2) Generates `gold.metric_embeddings` table DDL matching ARCHITECTURE.md Section 4, (3) Generates `gold.predictions` table DDL, (4) Generates `gold.graph_nodes` + `gold.graph_edges` tables (SQL fallback), (5) Generates `gold.reasoning_bank` table (empty prep for V1.3), (6) All generated SQL is syntactically valid (regex check for balanced parens, semicolons), (7) Hypertable creation calls are present for metric_embeddings and predictions, (8) Indexes are present for graph tables |
| **Exit criterion** | Generator produces complete, valid DDL for all four table groups. Unit tests verify DDL content. |
| **Risk level** | Low (follows established generator pattern) |
| **Estimated complexity** | M |

---

### Wave 3: Storage + Graph

#### P1-09: pgvector Extension in TimescaleDB Docker

| Field | Value |
|-------|-------|
| **Title** | Add pgvector to Pi TimescaleDB Docker image and integration environment |
| **Files to modify** | `deploy/pi/docker-compose.yml` (if TimescaleDB Dockerfile is referenced), `docker-compose.integration.yml` (if TimescaleDB service needs update). Note: the existing `docker/timescaledb/Dockerfile` already includes `postgresql-16-pgvector` but uses PG16. The Pi deployment uses `timescale/timescaledb:latest-pg15`. Need to verify PG version alignment and add pgvector init script. |
| **Files to create** | Init SQL script to run `CREATE EXTENSION IF NOT EXISTS vector;` and `CREATE SCHEMA IF NOT EXISTS gold;` (if not already present in init scripts) |
| **Dependencies** | None (infrastructure-only, can start early in Wave 3) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) `SELECT * FROM pg_extension WHERE extname = 'vector';` returns a row, (2) `SELECT '[1,2,3]'::vector;` succeeds (basic vector type works), (3) Integration environment starts without errors |
| **Exit criterion** | pgvector extension loads in both integration and Pi environments |
| **Risk level** | Low (pgvector is a pre-built apt package for arm64) |
| **Estimated complexity** | S |

**Important note on PG version**: The dev container `docker/timescaledb/Dockerfile` uses PG16. The Pi deployment uses PG15. The pgvector package name must match: `postgresql-15-pgvector` for Pi, `postgresql-16-pgvector` for dev. Verify which PG version the integration environment uses.

#### P1-10: StorageBackend Trait + PostgresStorage

| Field | Value |
|-------|-------|
| **Title** | Define StorageBackend trait and implement PostgresStorage for pgvector |
| **Files to create** | `crates/ndp-intelligence/src/storage/mod.rs` (trait), `crates/ndp-intelligence/src/storage/postgres.rs` (implementation) |
| **Files to modify** | `crates/ndp-intelligence/src/lib.rs` (wire up storage module) |
| **Dependencies** | P1-01 (crate exists), P1-03 (Embedding type), P1-09 (pgvector available for integration tests) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) Unit test: StorageBackend trait is object-safe, (2) Integration test: store_embedding writes to gold.metric_embeddings and load_embeddings reads it back, (3) Integration test: store_prediction writes to gold.predictions, (4) Integration test: get_pending_outcomes returns predictions without evaluated_at, (5) Integration test: record_outcome updates prediction with actual values, (6) Vector round-trip: stored Vec<f32> matches retrieved Vec<f32> exactly |
| **Exit criterion** | Integration tests pass against TimescaleDB with pgvector. All CRUD operations work. |
| **Risk level** | Medium (pgvector SQL syntax for vector operations, tokio-postgres integration) |
| **Estimated complexity** | L |

#### P1-11: GraphStore Trait + Backend

| Field | Value |
|-------|-------|
| **Title** | Define GraphStore trait and implement backend (ruvector-graph or SQL adjacency) |
| **Files to create** | `crates/ndp-intelligence/src/graph/mod.rs` (trait + dispatch), `crates/ndp-intelligence/src/graph/sql.rs` (SQL adjacency backend). Optionally: `crates/ndp-intelligence/src/graph/ruvector.rs` (if ruvector-graph compiles) |
| **Files to modify** | `crates/ndp-intelligence/src/lib.rs` (wire up graph module) |
| **Dependencies** | P1-01 (crate exists), Wave 0 outcome (determines backend). If SQL backend: P1-08 (DDL for graph tables) and P1-09 (integration env). |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) Unit test: GraphStore trait is object-safe, (2) add_node creates a node retrievable by get_neighbors, (3) add_edge creates a typed edge, (4) get_edges filters by edge_type correctly, (5) get_neighbors returns correct 1-hop set, (6) node_count and edge_count return correct values, (7) Duplicate node add is idempotent (upsert). For SQL backend: integration tests against TimescaleDB. For ruvector-graph backend: unit tests with in-memory graph. |
| **Exit criterion** | All CRUD + traversal tests pass. Backend choice documented in go/no-go report. |
| **Risk level** | Medium (backend selection depends on Wave 0 outcome) |
| **Estimated complexity** | L |

---

### Wave 4: Integration

#### P1-12: EmbeddingWriter Populator

| Field | Value |
|-------|-------|
| **Title** | Create EmbeddingWriter that pipes MetricEmbedder output to PostgresStorage |
| **Files to create** | `crates/ndp-lib/src/gold/populator/mod.rs` (Populator trait), `crates/ndp-lib/src/gold/populator/embedding_writer.rs` |
| **Files to modify** | `crates/ndp-lib/src/gold/mod.rs` (add `pub mod populator;` and re-exports) |
| **Dependencies** | P1-04 (MetricEmbedder), P1-10 (PostgresStorage), P1-09 (pgvector available) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) Integration test: construct GoldRow, embed via MetricEmbedder, write via EmbeddingWriter, read back from PostgresStorage, verify vector matches, (2) Integration test: batch of 10 rows processes without error, (3) Metadata is preserved (domain_id, bucket timestamp), (4) Duplicate bucket+domain_id is handled (upsert or error) |
| **Exit criterion** | Full round-trip integration test passes: GoldRow -> Embedding -> pgvector -> read back |
| **Risk level** | Medium (integration point between multiple components) |
| **Estimated complexity** | M |

#### P1-13: ndp-cli `gold intelligence` Subcommand

| Field | Value |
|-------|-------|
| **Title** | Add `ndp gold intelligence schema` subcommand that generates pgvector DDL |
| **Files to modify** | `tools/ndp-cli/src/commands/gold.rs` (add `Intelligence` variant to `GoldCommands` enum), `tools/ndp-cli/Cargo.toml` (may need ndp-lib features) |
| **Dependencies** | P1-08 (PgVectorSchemaGenerator) |
| **Agent type** | `ndp-rust-dev` |
| **Test requirements** | (1) `ndp gold intelligence schema --domain indoor-air-quality` outputs valid SQL, (2) Output includes CREATE EXTENSION, metric_embeddings, predictions, graph tables, reasoning_bank, (3) `ndp gold intelligence schema --help` shows usage, (4) Missing --domain produces helpful error |
| **Exit criterion** | CLI outputs valid, complete intelligence DDL to stdout |
| **Risk level** | Low (follows established gold subcommand pattern) |
| **Estimated complexity** | M |

---

## 3. Critical Path Analysis

```
Wave 0 ─────────> Wave 1 ──────────> Wave 2 ──────────> Wave 4
P0-01             P1-01               P1-04               P1-12
P0-02             P1-02               P1-07               P1-13
P0-03             P1-03               P1-08
P0-04             P1-05                 │
P0-05             P1-06                 │
                    │                   │
                    └──────> Wave 3 ────┘
                             P1-09
                             P1-10
                             P1-11
```

### Critical Path (longest sequential chain):

```
P0-01 -> P0-02 -> P0-03 -> P0-05 -> P1-01 -> P1-03 -> P1-05 -> P1-04 -> P1-12
                                                                    |
                                              P1-09 -> P1-10 ------+
```

**Critical path tasks** (any delay = overall delay):

1. **P0-01** (compile gate) -- blocks everything
2. **P0-02** (run gate) -- blocks everything
3. **P0-05** (go/no-go report) -- blocks Wave 1 crate Cargo.toml decisions
4. **P1-01** (crate creation) -- blocks all Wave 1 ndp-lib work
5. **P1-03** (Embedder trait) -- blocks P1-04 (MetricEmbedder) and P1-05 (RunningStats module location)
6. **P1-05** (RunningStats) -- blocks P1-04
7. **P1-04** (MetricEmbedder) -- blocks P1-12 (EmbeddingWriter)
8. **P1-09** (pgvector Docker) -- blocks P1-10 (PostgresStorage integration tests)
9. **P1-10** (PostgresStorage) -- blocks P1-12 (EmbeddingWriter)
10. **P1-12** (EmbeddingWriter) -- final integration deliverable

### Off Critical Path:

- **P0-03, P0-04**: Can run in parallel with each other after P0-02
- **P1-02**: Binary crate, independent of ndp-lib work
- **P1-06**: Config types, independent of P1-03/P1-05
- **P1-07**: DomainConfig extension, can start as soon as P1-06 finishes
- **P1-08**: DDL generator, can start as soon as P1-06 finishes
- **P1-11**: Graph backend, parallel with P1-10
- **P1-13**: CLI subcommand, parallel with P1-12

### Schedule Optimization

Wave 2 and Wave 3 can overlap significantly:
- P1-09 (Docker pgvector) can start as soon as Wave 0 completes, before Wave 1 finishes
- P1-07 and P1-08 can start as soon as P1-06 finishes (mid-Wave 1)
- P1-10 and P1-11 can start as soon as P1-01 + P1-03 + P1-09 are done

With 2-3 agents, the effective timeline compresses to:

| Week | Agent A | Agent B | Agent C (optional) |
|------|---------|---------|-------------------|
| 0 (day) | Wave 0: P0-01..P0-05 | -- | -- |
| 1 | P1-01, P1-03, P1-05 | P1-02, P1-06, P1-09 | -- |
| 2 | P1-04 | P1-07, P1-08 | P1-10, P1-11 |
| 3 | P1-12 | P1-13 | Test sweep |

---

## 4. GitHub Issue Plan

### Wave 0

```
Title: [fe-003] Wave 0: Go/No-Go Gate — ruvector aarch64 compilation proof
Labels: implementation, fe-003, wave-0, phase-0
Body:
## Objective
Prove ruvector-core and ruvector-graph compile and run on aarch64 (Pi 5).

## Acceptance Criteria
- [ ] P0-01: Minimal project with ruvector-core + ruvector-graph compiles
- [ ] P0-02: Binary runs on Pi 5 without crash
- [ ] P0-03: ruvector-core smoke test (100 vectors, K-NN search) passes
- [ ] P0-04: ruvector-graph smoke test (nodes, edges, traversal) passes
- [ ] P0-05: Performance measurements documented in go/no-go report

## SPARC Docs
`product/features/fe-003/`

## Artifact
`product/features/fe-003/reports/phase0-go-no-go.md`
```

### Wave 1

```
Title: [fe-003] Wave 1: Foundation types — crate skeleton, Embedder trait, RunningStats, config types
Labels: implementation, fe-003, wave-1, phase-1
Body:
## Objective
Establish crate structure, core traits, and pure-Rust types with no DB dependencies.

## Acceptance Criteria
- [ ] P1-01: crates/ndp-intelligence compiles as workspace member
- [ ] P1-02: apps/ndp-intelligence-app --help works with daemon/one-shot/backfill/status subcommands
- [ ] P1-03: Embedder trait + GoldRow + Embedding types with unit tests
- [ ] P1-05: RunningStats with statistical accuracy tests (< 1% relative error)
- [ ] P1-06: EmbeddingConfig types deserialize from ARCHITECTURE.md Section 6 JSON

## SPARC Docs
`product/features/fe-003/`

## Dependencies
Wave 0 must complete (determines ruvector feature flags)
```

### Wave 2

```
Title: [fe-003] Wave 2: Config + generators — MetricEmbedder, DomainConfig intelligence, PgVector DDL
Labels: implementation, fe-003, wave-2, phase-1
Body:
## Objective
Implement MetricEmbedder, extend DomainConfig, and build PgVector schema DDL generator.

## Acceptance Criteria
- [ ] P1-04: MetricEmbedder produces correct embeddings for known inputs
- [ ] P1-07: DomainConfig intelligence field works with zero test regressions
- [ ] P1-08: PgVectorSchemaGenerator produces valid DDL for all 4 table groups

## SPARC Docs
`product/features/fe-003/`

## Dependencies
Wave 1 must complete
```

### Wave 3

```
Title: [fe-003] Wave 3: Storage + graph — pgvector Docker, PostgresStorage, GraphStore
Labels: implementation, fe-003, wave-3, phase-1
Body:
## Objective
Build durable storage and graph backends. Requires integration environment.

## Acceptance Criteria
- [ ] P1-09: pgvector extension loads in TimescaleDB container
- [ ] P1-10: PostgresStorage read/write round-trip passes for embeddings and predictions
- [ ] P1-11: GraphStore CRUD + traversal tests pass

## SPARC Docs
`product/features/fe-003/`

## Dependencies
Wave 1 must complete. Integration environment required.
```

### Wave 4

```
Title: [fe-003] Wave 4: Integration — EmbeddingWriter, CLI intelligence subcommand
Labels: implementation, fe-003, wave-4, phase-1
Body:
## Objective
Wire MetricEmbedder output to PostgresStorage via EmbeddingWriter. Add CLI subcommand.

## Acceptance Criteria
- [ ] P1-12: GoldRow -> Embedding -> pgvector -> read back round-trip passes
- [ ] P1-13: `ndp gold intelligence schema --domain indoor-air-quality` outputs valid SQL

## SPARC Docs
`product/features/fe-003/`

## Dependencies
Wave 2 + Wave 3 must complete. Integration environment required.
```

---

## 5. Test Strategy

### Unit Test Coverage

Every new module requires tests. Minimum coverage targets per task:

| Task | Module | Required Unit Tests | Runs In |
|------|--------|-------------------|---------|
| P1-03 | `gold::embeddings::mod` | GoldRow construction, Embedding construction, trait object safety | Dev container |
| P1-04 | `gold::embeddings::metric` | Temporal encoding (6 cases), z-score normalization (3 cases), NULL handling (3 strategies), golden test (5+ rows), dimension count | Dev container |
| P1-05 | `gold::embeddings::stats` | Mean convergence, std convergence, exponential decay, z-score output, warmup handling | Dev container |
| P1-06 | `gold::embeddings::config` | IntelligenceConfig deser, EmbeddingConfig deser, SearchConfig deser, None handling, round-trip | Dev container |
| P1-07 | `gold::config::domain` | Existing tests pass (regression), new intelligence field deser, None default | Dev container |
| P1-08 | `gold::generators::pgvector_schema` | DDL content for all 4 tables, hypertable calls, index creation, extension creation | Dev container |
| P1-11 | `intelligence::graph` | Trait object safety, node CRUD, edge CRUD, traversal, counts | Dev container (SQL backend needs integration env) |
| P1-13 | CLI gold command | --help output, schema subcommand argument parsing | Dev container |

### Integration Test Requirements

These tests require TimescaleDB with pgvector running:

| Task | Test | What It Validates |
|------|------|------------------|
| P1-09 | Extension load | `SELECT * FROM pg_extension WHERE extname = 'vector'` |
| P1-10 | Embedding CRUD | store_embedding + load_embeddings round-trip |
| P1-10 | Prediction CRUD | store_prediction + get_pending_outcomes + record_outcome |
| P1-10 | Vector accuracy | Stored Vec<f32> matches retrieved Vec<f32> exactly |
| P1-11 (SQL) | Graph CRUD | Node/edge INSERT + SELECT via gold.graph_nodes/edges |
| P1-12 | Full pipeline | GoldRow -> MetricEmbedder -> EmbeddingWriter -> PostgresStorage -> verify |

Integration tests should be gated behind a `#[cfg(feature = "integration")]` feature flag or `#[ignore]` attribute, runnable via:
```bash
# In integration environment
docker compose -f docker-compose.integration.yml up -d
TIMESCALE_URL="postgresql://..." cargo test --features integration
```

### Backward Compatibility Tests

Critical for P1-07 (DomainConfig extension):

1. **Existing DomainConfig tests** in `crates/ndp-lib/src/gold/config/domain.rs` -- all must pass unchanged
2. **ndp-validate tests** (`cargo test -p ndp-validate`) -- all 65 tests must pass
3. **ndp-gold-ddl tests** (`cargo test -p ndp-gold-ddl`) -- all 15 tests must pass
4. **Full workspace test** (`cargo test --workspace`) -- 904+ tests must pass

Run the full workspace test suite after P1-07 merges. Any failure is a blocker.

### Environment Requirements

| Test Category | Environment | How to Run |
|---------------|-------------|-----------|
| Unit tests (all tasks) | Dev container (codespace) | `cargo test -p ndp-lib`, `cargo test -p ndp-intelligence` |
| Integration tests (P1-09, P1-10, P1-11 SQL, P1-12) | `docker-compose.integration.yml` | `docker compose -f docker-compose.integration.yml up -d && cargo test --features integration` |
| Backward compat (P1-07) | Dev container | `cargo test --workspace` |
| Go/no-go (Wave 0) | Pi 5 native or cross-compilation | Native `cargo build --release` on Pi, or `cross build --target aarch64-unknown-linux-gnu` |

---

## 6. Risk Register

### R1: ruvector-core Fails to Compile on aarch64

| Field | Value |
|-------|-------|
| **Phase** | 0 |
| **Likelihood** | Medium |
| **Impact** | Medium |
| **Mitigation** | (1) Try `default-features = false, features = ["storage", "hnsw", "parallel"]` to disable SimSIMD. (2) If that fails, proceed with pgvector-only mode -- SimilarityEngine trait is implemented by PgVectorEngine as well as HnswEngine. Functionally equivalent, ~10x slower search (still < 10ms for < 10K vectors). |
| **Detection** | Wave 0, P0-01 |
| **Contingency tasks affected** | P1-01 Cargo.toml (remove ruvector-core dep), P1-10 (PostgresStorage becomes primary search backend), P1-11 (must use SQL adjacency) |

### R2: ruvector-graph Fails to Compile on aarch64

| Field | Value |
|-------|-------|
| **Phase** | 0 |
| **Likelihood** | Low (pure Rust) |
| **Impact** | Low |
| **Mitigation** | SQL adjacency tables (`gold.graph_nodes`, `gold.graph_edges`) provide identical GraphStore interface. P1-08 already generates DDL for these tables. P1-11 implements SqlGraphStore as a backend. |
| **Detection** | Wave 0, P0-04 |
| **Contingency tasks affected** | P1-11 only (use SQL backend instead of ruvector backend) |

### R3: pgvector apt Package Unavailable for ARM64

| Field | Value |
|-------|-------|
| **Phase** | 1 |
| **Likelihood** | Very Low |
| **Impact** | High (blocks Wave 3 entirely) |
| **Mitigation** | (1) pgvector provides pre-built arm64 packages -- this is well-established. (2) If unavailable, compile from source: `git clone pgvector && make && make install` (C extension, compiles in < 1 minute). (3) The dev container Dockerfile already includes `postgresql-16-pgvector` successfully. |
| **Detection** | Wave 3, P1-09 |

### R4: DomainConfig Deserialization Breakage

| Field | Value |
|-------|-------|
| **Phase** | 1 |
| **Likelihood** | Low |
| **Impact** | High (breaks existing Gold DDL pipeline) |
| **Mitigation** | (1) The `intelligence` field uses `#[serde(default)]` and `Option<IntelligenceConfig>`, so existing JSON without this field deserializes to `None`. (2) Run full workspace test suite (`cargo test --workspace`) after P1-07. (3) Do NOT change any existing DomainConfig fields -- only add the new optional field. (4) Test with both the current `domain.json` (no intelligence) and the extended version. |
| **Detection** | Wave 2, P1-07 backward compatibility tests |

### R5: Workspace Compilation Time Increase

| Field | Value |
|-------|-------|
| **Phase** | 1 |
| **Likelihood** | High (certainty) |
| **Impact** | Low |
| **Mitigation** | (1) ruvector-core adds SimSIMD (C compilation) and redb. Expected +30-60s on full rebuild. (2) Incremental builds unaffected (ruvector-core only recompiles when its version changes). (3) `ndp-intelligence` is a separate crate -- changes to it do not trigger recompilation of `air-quality-app` or other binaries. (4) Docker build uses cargo-chef caching, so ruvector-core is only compiled once per dependency version change. |
| **Detection** | Wave 1, P1-01 first build |

### R6: pgvector SQL Syntax for Vector Operations

| Field | Value |
|-------|-------|
| **Phase** | 1 |
| **Likelihood** | Medium |
| **Impact** | Low |
| **Mitigation** | pgvector uses `::vector` cast and `<->` operator for L2 distance, `<=>` for cosine distance. tokio-postgres requires raw SQL queries. (1) Reference pgvector documentation for correct SQL syntax. (2) Use parameterized queries with vector format: `SELECT ... FROM gold.metric_embeddings ORDER BY embedding <=> $1::vector LIMIT $2`. (3) Test vector round-trip early in P1-10 to catch SQL issues. |
| **Detection** | Wave 3, P1-10 first integration test |

### R7: ruvector-core API Instability

| Field | Value |
|-------|-------|
| **Phase** | 1+ |
| **Likelihood** | Medium (13 versions in 3 months) |
| **Impact** | Medium |
| **Mitigation** | (1) Pin exact version `2.0.1` in Cargo.toml. (2) Wrap all ruvector API calls behind `SimilarityEngine` trait -- if API changes, only `HnswEngine` wrapper needs updating. (3) Consider vendoring the crate if breakage occurs. |
| **Detection** | Any `cargo update` or version bump |

### R8: PG Version Mismatch (PG15 vs PG16)

| Field | Value |
|-------|-------|
| **Phase** | 1 |
| **Likelihood** | Medium |
| **Impact** | Low |
| **Mitigation** | The dev container uses PG16, Pi uses PG15. pgvector package names differ (`postgresql-15-pgvector` vs `postgresql-16-pgvector`). (1) Verify which PG version the integration compose file uses. (2) Use correct package name per environment. (3) pgvector SQL syntax is identical across PG versions. |
| **Detection** | Wave 3, P1-09 |

---

## 7. File Inventory

### New Files (Phase 0 + Phase 1)

```
product/features/fe-003/reports/phase0-go-no-go.md           # P0-05

crates/ndp-intelligence/Cargo.toml                            # P1-01
crates/ndp-intelligence/src/lib.rs                            # P1-01
crates/ndp-intelligence/src/config.rs                         # P1-01
crates/ndp-intelligence/src/error.rs                          # P1-01
crates/ndp-intelligence/src/similarity/mod.rs                 # P1-01
crates/ndp-intelligence/src/graph/mod.rs                      # P1-01, P1-11
crates/ndp-intelligence/src/graph/sql.rs                      # P1-11
crates/ndp-intelligence/src/graph/ruvector.rs                 # P1-11 (conditional)
crates/ndp-intelligence/src/predictions/mod.rs                # P1-01
crates/ndp-intelligence/src/storage/mod.rs                    # P1-01, P1-10
crates/ndp-intelligence/src/storage/postgres.rs               # P1-10

apps/ndp-intelligence-app/Cargo.toml                          # P1-02
apps/ndp-intelligence-app/src/main.rs                         # P1-02

crates/ndp-lib/src/gold/embeddings/mod.rs                     # P1-03
crates/ndp-lib/src/gold/embeddings/metric.rs                  # P1-03, P1-04
crates/ndp-lib/src/gold/embeddings/stats.rs                   # P1-05
crates/ndp-lib/src/gold/embeddings/config.rs                  # P1-06

crates/ndp-lib/src/gold/generators/pgvector_schema.rs         # P1-08

crates/ndp-lib/src/gold/populator/mod.rs                      # P1-12
crates/ndp-lib/src/gold/populator/embedding_writer.rs         # P1-12
```

### Modified Files

```
Cargo.toml (workspace root)                                   # P1-01, P1-02
crates/ndp-lib/src/gold/mod.rs                                # P1-03, P1-08, P1-12
crates/ndp-lib/src/gold/config/domain.rs                      # P1-07
crates/ndp-lib/src/gold/generators/mod.rs                     # P1-08
crates/ndp-lib/src/gold/embeddings/mod.rs                     # P1-05, P1-06 (after P1-03 creates it)
config/domains/indoor-air-quality/domain.json                 # P1-07
tools/ndp-cli/src/commands/gold.rs                            # P1-13
```

---

## 8. Dependency Graph (Mermaid)

```
P0-01 --> P0-02 --> P0-03
                --> P0-04
P0-03 --> P0-05
P0-04 --> P0-05

P0-05 --> P1-01
P0-05 --> P1-02

P1-01 --> P1-03
P1-01 --> P1-02

P1-03 --> P1-05
P1-03 --> P1-04
P1-03 --> P1-06

P1-05 --> P1-04
P1-06 --> P1-04
P1-06 --> P1-07
P1-06 --> P1-08

P1-01 --> P1-10
P1-01 --> P1-11
P1-03 --> P1-10

P1-09 --> P1-10
P1-09 --> P1-11 (SQL backend)

P1-04 --> P1-12
P1-10 --> P1-12

P1-08 --> P1-13
```

---

*Task decomposition for fe-003 Phase 0 + Phase 1. 18 tasks across 5 waves. Critical path runs through compilation gate, trait definition, MetricEmbedder, and storage integration. Estimated 3 weeks with 2-3 agents, 1 day for Wave 0 gate.*
