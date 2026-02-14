# RuVector Pi 5 Compilation Feasibility Analysis

**Research Date:** 2026-02-10
**Platform:** Raspberry Pi 5 (16GB RAM, 1TB NVMe SSD, Cortex-A76 quad-core @ 2.4GHz)
**Target:** Native Rust compilation for aarch64-unknown-linux-gnu
**Method:** 3-agent research swarm (crate ecosystem, SimSIMD ARM64, pgrx ARM64)

---

## Executive Summary

RuVector's Rust crates can compile on Pi 5 (aarch64), but **no pre-built Docker images exist for arm64** — both `ruvnet/ruvector` and `ruvnet/ruvector-postgres` on Docker Hub are amd64-only. NDP would need to compile from source. The core dependency chain (SimSIMD + hnsw_rs + redb) is ARM64-compatible with confirmed NEON support. The recommended entry point is `ruvector-core` as a Cargo dependency, not the WASM-oriented `rvlite` nor the pre-built Docker images.

**Key finding:** `rvlite` (marketed as "edge") is a WASM/browser target — it uses `wasm-bindgen` and IndexedDB persistence. For native Rust on Pi, use `ruvector-core` directly.

---

## 1. Docker Hub Images: amd64 Only

### ruvnet/ruvector

| Tag | Architecture | Size | Last Pushed |
|-----|-------------|------|-------------|
| latest | amd64 only | 252 MB | 2025-12-06 |
| 0.2.5 | amd64 only | 252 MB | 2025-12-06 |

### ruvnet/ruvector-postgres

| Tag | Architecture | Size | Last Pushed |
|-----|-------------|------|-------------|
| latest | amd64 only | 331.9 MB | 2026-01-28 |
| 2.0.1 | amd64 only | 331.9 MB | 2026-01-28 |
| 2.0.0 | amd64 only | 332.8 MB | — |
| 0.2.7 | amd64 only | 332.1 MB | — |
| 0.2.6 | amd64 only | 170.3 MB | — |
| 0.2.5 | amd64 only | 158.5 MB | — |
| 0.2.4 | amd64 only | 170.1 MB | — |

**Verdict:** Neither image runs on Pi 5. QEMU emulation would be extremely slow. Must compile from source.

---

## 2. Crate Ecosystem Inventory

76 crates in the ruvector workspace. Relevant to NDP:

| Crate | Version | Downloads | ARM64 | Purpose |
|-------|---------|-----------|-------|---------|
| **ruvector-core** | 2.0.1 | 3,648 | **Yes** | HNSW + redb + SimSIMD — main entry point |
| **ruvector-graph** | 2.0.1 | 1,390 | **Yes** (pure Rust) | Neo4j-compatible hypergraph |
| **ruvector-gnn** | 2.0.1 | 1,066 | **Yes** (pure Rust) | GNN layer on HNSW topology |
| **ruvector-postgres** | 2.0.1 | 230 | **Yes** (needs pgrx build) | 230+ SQL functions, PG15 compatible |
| **ruvector-attention** | 0.1.31 | 604 | **Yes** (pure Rust) | Geometric/graph/sparse attention |
| **ruvector-cluster** | 2.0.1 | 412 | **Yes** (pure Rust) | Distributed clustering/sharding |
| **ruvector-metrics** | 0.1.30 | 424 | **Yes** (pure Rust) | Prometheus-compatible metrics |
| **ruvector-server** | 0.1.30 | 167 | **Yes** (pure Rust) | REST API server |
| **rvlite** | 0.3.0 | 22 | **No — WASM only** | wasm-bindgen + IndexedDB |
| **ruvector-onnx-embeddings** | 0.1.0 | 66 | **Problematic** | ONNX Runtime arm64 binaries uncertain |
| **ruvllm** | 2.0.2 | 310 | Untested | LLM runtime — unnecessary for NDP |
| **sona** | — | — | **Yes** (pure Rust) | LoRA + EWC++ — in workspace |

### Workspace Configuration

- Rust edition: 2021
- Minimum Rust version: **1.77 stable** (nightly NOT required)
- Resolver: version 2
- Release profile: LTO enabled, single codegen unit, symbols stripped
- License: MIT (core crates), MIT OR Apache-2.0 (rvlite)

### Why NOT rvlite

rvlite is a WASM wrapper around `ruvector-core` with `memory-only` features:
- API surface uses `wasm-bindgen` and returns `Result<T, JsValue>`
- Persistence is IndexedDB (browser-only via `web-sys`)
- No file-based storage, no redb, no SimSIMD
- SQL/Cypher/SPARQL parsers present but some features are stubs

For native Rust on Pi 5, `ruvector-core` with default features is the correct dependency.

---

## 3. Dependency Chain Analysis

### ruvector-core Default Features

| Feature | Default | What It Gates | ARM64 Compatible |
|---------|---------|---------------|-----------------|
| `simd` | Yes | SimSIMD distance functions | **Yes** (NEON) |
| `storage` | Yes | redb persistence + agenticdb | **Yes** |
| `hnsw` | Yes | hnsw_rs index | **Yes** |
| `api-embeddings` | Yes | reqwest HTTP client | **Yes** |
| `parallel` | Yes | rayon + crossbeam | **Yes** |
| `memory-only` | No | Pure in-memory, no storage/hnsw/simd | Yes (fallback) |

### Critical Dependencies

#### SimSIMD v5.9+ (C via FFI)

**Status: ARM64 COMPATIBLE — NEON confirmed**

- C library with Rust bindings via `cc` crate build.rs
- Explicitly supports ARM NEON, SVE, SVE2
- Runtime CPU feature detection probes ARM system registers
- Pi 5 Cortex-A76 (ARMv8.2-A): NEON = yes, SVE = no — correctly auto-detected
- Requires `gcc` at build time (`build-essential` in Docker)
- Graceful fallback: if backends fail, disables one-by-one until compilation succeeds
- Can override via env vars: `SIMSIMD_TARGET_SVE=0` to force-disable

Performance (from SimSIMD benchmarks, AWS Graviton3):

| Operation | x86 (Sapphire Rapids) | ARM (Graviton3) |
|-----------|----------------------|-----------------|
| Cosine f32, 1536d | 8,202,910 ops/s | 3,400,620 ops/s |
| Euclidean i8 | 18,989,000 ops/s | 18,878,200 ops/s |

Pi 5 Cortex-A76 will be somewhat slower than Graviton3 but NEON acceleration still provides significant speedup over scalar. Integer (i8) operations perform at near x86 parity.

#### hnsw_rs v0.3 (Pure Rust)

**Status: FULLY COMPATIBLE**

- No C/C++ FFI in core algorithm
- Two optional SIMD features: `simdeez_f` (x86_64 only — do NOT enable) and `stdsimd` (nightly only)
- Without SIMD features: scalar distance calculations, works on any architecture
- Workspace applies a local patch for WASM getrandom — does not affect native ARM builds

#### redb v2.1 (Pure Rust)

**Status: FULLY COMPATIBLE**

- Pure Rust embedded database, 3M+ downloads, mature
- No native/FFI dependencies, compiles for any target

#### pgrx v0.12 (for ruvector-postgres only)

**Status: COMPATIBLE — aarch64 officially tested**

- README explicitly lists aarch64 Linux as a tested platform
- Supports PG13-PG17 (NDP uses PG15 — confirmed compatible)
- Requires: libclang 11+, PostgreSQL development headers, C compiler
- **Cross-compilation limitation:** cshim feature not supported for cross-compile — native arm64 build is safer
- v0.12.9 blocklists specific PG patch versions (15.9) due to ABI issues — verify TimescaleDB's PG15 minor version
- Build time: 15-30 minutes natively on Pi 5

#### fastembed / ruvector-onnx-embeddings (ONNX Runtime)

**Status: PROBLEMATIC — skip for Pi**

- Default feature downloads pre-built ONNX Runtime binaries — arm64 availability uncertain
- Alternative `ort-load-dynamic` feature uses system ONNX Runtime
- **Not needed:** NDP generates numerical feature vectors from Gold aggregates, no LLM embedding model required

#### Other Dependencies (all pure Rust, fully compatible)

- rayon / crossbeam (parallelism)
- rkyv / bincode / serde (serialization)
- reqwest with rustls-tls (HTTP client, pure Rust TLS)
- tokio (async runtime)

---

## 4. Three Deployment Paths

### Path A: ruvector-core as Cargo Dependency (Recommended for Phase 1)

```toml
# crates/ndp-intelligence/Cargo.toml
[dependencies]
ruvector-core = { version = "2.0.1" }
```

Compiles as part of NDP's existing Docker build. Add `build-essential` to the builder stage for SimSIMD's C compilation.

| Aspect | Detail |
|--------|--------|
| Build change | Add `apt-get install -y build-essential` to Dockerfile builder stage |
| Memory | ~50-100MB for index + service |
| Effort | 1-2 weeks integration |
| What you get | VectorDB, HNSW index, redb persistence, f32/f16/PQ8, cosine/euclidean/dot with NEON |
| What you don't get | SQL interface, GNN, graph queries (those are separate crates) |
| Architecture fit | New `ndp-intelligence` crate, follows hexagonal pattern |

To add GNN and graph capabilities:

```toml
ruvector-core = { version = "2.0.1" }
ruvector-graph = { version = "2.0.1" }
ruvector-gnn = { version = "2.0.1" }
```

### Path B: ruvector-postgres Extension in TimescaleDB

Build pgrx extension for arm64, install into existing TimescaleDB container.

```dockerfile
# Custom TimescaleDB + ruvector extension
FROM rust:1-bookworm AS ext-builder
RUN apt-get update && apt-get install -y \
    libclang-dev build-essential pkg-config libssl-dev \
    postgresql-server-dev-15 libreadline-dev zlib1g-dev \
    flex bison libxml2-dev libxslt-dev
RUN cargo install cargo-pgrx --version 0.12.9 --locked
RUN cargo pgrx init --pg15=$(which pg_config)
WORKDIR /build
COPY crates/ruvector-postgres/ .
RUN cargo pgrx package --pg-config=$(which pg_config) --features pg15

FROM timescale/timescaledb:latest-pg15
COPY --from=ext-builder /build/target/release/ruvector-postgres-pg15/ \
     /usr/share/postgresql/15/extension/
COPY --from=ext-builder /build/target/release/ruvector-postgres-pg15/lib/ \
     /usr/lib/postgresql/15/lib/
```

| Aspect | Detail |
|--------|--------|
| Build | 15-30 min native on Pi, 1-2 hours via QEMU `docker buildx --platform linux/arm64` |
| Memory | +50-100MB inside TimescaleDB process |
| Effort | 2-3 weeks (custom Docker image, init scripts, testing) |
| What you get | 230+ SQL functions, pgvector-compatible operators, co-located with Gold data |
| What you don't get | GNN, SONA, ReasoningBank (separate crates, not exposed via PG extension) |
| Risk | pgrx cshim cross-compilation not supported — prefer native arm64 build |

### Path C: Full ruvector Container (Custom arm64 Build)

Build `ruvector-server` + dependencies as a standalone Docker container for arm64.

| Aspect | Detail |
|--------|--------|
| Build | Full workspace compilation for aarch64, needs arm64 CI runner or native Pi build |
| Memory | ~512MB-1GB |
| Effort | 3-4 weeks (Dockerfile, arm64 pipeline, integration testing) |
| What you get | HTTP/gRPC API, full feature set |
| What you don't get | Pre-built images — you own the build pipeline |

---

## 5. The pgvector Alternative (Zero-Build Baseline)

pgvector is available as a **pre-built arm64 apt package** and provides basic HNSW vector search with zero compilation:

```dockerfile
FROM timescale/timescaledb:latest-pg15
RUN apt-get update && apt-get install -y postgresql-15-pgvector && rm -rf /var/lib/apt/lists/*
```

| | pgvector | ruvector-postgres | ruvector-core (Cargo) |
|---|---|---|---|
| arm64 install | `apt-get install` | Build from source | `cargo build` |
| Build effort | **Zero** | Significant | Moderate |
| SQL functions | ~15 | 230+ | None (Rust API) |
| HNSW index | Yes | Yes | Yes |
| Distance metrics | 3 | 6+ | 6+ |
| GNN/Graph | No | Some | Via ruvector-graph |
| Attention | No | Some | Via ruvector-attention |
| Quantization | half-vector, binary | scalar, product, binary | scalar, product, binary |
| SONA/EWC++ | No | No | Via sona crate |

NDP's `docker/timescaledb/Dockerfile` already references pgvector (line 8: `postgresql-16-pgvector`). Adapting this for the Pi's PG15 is trivial.

**pgvector is a viable Phase 1 starting point** if the goal is validating the sensor fingerprinting thesis before investing in ruvector compilation.

---

## 6. Maturity Assessment

### Positive Signals

- Active development: 13 versions of ruvector-core since Nov 2025
- Comprehensive module coverage: 76 crates
- Uses battle-tested dependencies (redb: 3M+ downloads, SimSIMD: 101K/month)
- MIT licensed
- Pure Rust core with optional C acceleration
- Rust 1.77 stable — no nightly required
- 85% documentation coverage on ruvector-core

### Red Flags

| Concern | Severity | Mitigation |
|---------|----------|------------|
| Very low adoption (3,648 downloads total) | High | Underlying deps are proven; ruvector is the integration layer |
| No arm64 CI visible | High | Go/no-go test: `cargo build` with ruvector-core targeting aarch64 |
| 13 versions in 3 months (API instability) | Medium | Pin exact version, vendor if needed |
| No GitHub issues for ARM/Pi | Medium | Could mean it works or nobody tried — test first |
| AgenticDB uses placeholder hash embeddings | Low | NDP brings its own numerical embeddings |
| rvlite is WASM-only despite "edge" marketing | Low | Use ruvector-core instead |
| fastembed ONNX arm64 uncertain | Low | Not needed — NDP uses numerical vectors |
| Version jump from 0.x to 2.0.x | Low | Marketing version, not maturity indicator |

---

## 7. Build Requirements Summary

### For Path A (Cargo dependency — recommended)

```
Rust toolchain:  >= 1.77 stable (NDP uses 1.83)
C compiler:      gcc (for SimSIMD) — add build-essential to Dockerfile
Nightly:         NOT required
Cross-compile:   Set CC=aarch64-linux-gnu-gcc, or build natively on Pi
Docker change:   Add build-essential to builder stage
Feature flags:   All defaults work on arm64
```

### For Path B (PG extension)

```
Everything from Path A, plus:
libclang:        >= 11 (for pgrx bindgen)
PG dev headers:  postgresql-server-dev-15
cargo-pgrx:      0.12.9
Build method:    Native on Pi (15-30 min) or arm64 CI runner
                 Cross-compilation has cshim limitation — native preferred
PG15 version:    Avoid 15.9 specifically (pgrx blocklist)
```

### For Path C (Full container)

```
Everything from Path A, plus:
Docker buildx:   For multi-arch image
Build host:      arm64 runner (GitHub Actions, or Pi itself)
Build time:      30-60 min native, 1-2 hours under QEMU
```

---

## 8. Recommended Go/No-Go Test

Before committing to any integration path, run this 30-minute validation:

```bash
# 1. Create a minimal test project
cargo init /tmp/ruvector-arm-test
cd /tmp/ruvector-arm-test

# 2. Add ruvector-core
cat >> Cargo.toml << 'EOF'
[dependencies]
ruvector-core = "2.0.1"
EOF

# 3. Write a minimal smoke test
cat > src/main.rs << 'EOF'
use ruvector_core::VectorDB;
fn main() {
    println!("ruvector-core compiled successfully on aarch64");
    // Attempt to create a VectorDB instance
    // and insert/search a single vector
}
EOF

# 4. Cross-compile (from dev machine)
cargo build --target aarch64-unknown-linux-gnu --release

# OR native compile (on Pi 5)
cargo build --release
```

If this compiles and the smoke test passes on Pi 5 with NEON acceleration, the entire Path A integration is de-risked. If SimSIMD fails, retry with:

```toml
ruvector-core = { version = "2.0.1", default-features = false, features = ["storage", "hnsw", "parallel"] }
```

This disables SimSIMD and falls back to scalar distance calculations.

---

## 9. Recommendation

| Phase | Approach | Why |
|-------|----------|-----|
| **Immediate** | Run go/no-go test (30 min) | De-risk arm64 compilation before any design work |
| **Phase 1** | `ruvector-core` as Cargo dep in `ndp-intelligence` crate | Minimal build friction, follows existing architecture, validates HNSW + NEON on Pi |
| **Phase 1 alt** | pgvector in TimescaleDB (zero build risk) | If go/no-go fails, or for SQL-native path alongside Cargo approach |
| **Phase 2** | Add `ruvector-graph` + `ruvector-gnn` | When causal knowledge graph is needed |
| **Phase 2 alt** | Build ruvector-postgres for PG15 arm64 | If SQL-native vector operations prove more ergonomic |
| **Phase 3** | Custom arm64 Docker image with full ruvector | If standalone server benefits outweigh build complexity |

**Do not use the Docker Hub images.** They are amd64-only. NDP would be compiling from source regardless of deployment path.

---

## Sources

- [ruvector-core on crates.io](https://crates.io/crates/ruvector-core) — v2.0.1, 3,648 downloads
- [ruvector-postgres on crates.io](https://crates.io/crates/ruvector-postgres) — v2.0.1, 230 downloads
- [rvlite on crates.io](https://crates.io/crates/rvlite) — v0.3.0, 22 downloads
- [SimSIMD on crates.io](https://crates.io/crates/simsimd) — v6.5.12, 101K/month
- [SimSIMD GitHub](https://github.com/ashvardanian/SimSIMD) — ARM NEON/SVE documentation
- [hnsw_rs GitHub](https://github.com/jean-pierreBoth/hnswlib-rs) — Pure Rust HNSW
- [pgrx GitHub](https://github.com/pgcentralfoundation/pgrx) — aarch64 Linux tested
- [pgrx CROSS_COMPILE.md](https://github.com/pgcentralfoundation/pgrx/blob/master/CROSS_COMPILE.md) — cshim limitation
- [RuVector GitHub](https://github.com/ruvnet/ruvector) — 76-crate workspace
- [ruvnet/ruvector Docker Hub](https://hub.docker.com/r/ruvnet/ruvector) — amd64 only
- [ruvnet/ruvector-postgres Docker Hub](https://hub.docker.com/r/ruvnet/ruvector-postgres) — amd64 only
- [redb on crates.io](https://crates.io/crates/redb) — Pure Rust, 3M+ downloads
- [pgvector GitHub](https://github.com/pgvector/pgvector) — Pre-built arm64 packages available

---

*Research conducted by 3-agent swarm (crate ecosystem, SimSIMD ARM64, pgrx ARM64). Synthesis by coordinator.*
*All findings are based on published crate metadata, documentation, and Docker Hub manifests as of 2026-02-10.*
*No compilation was attempted — the go/no-go test should be run before design decisions.*
