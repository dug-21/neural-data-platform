# ADR-006-002: Binary Architecture

**Feature**: dp-006 (Silver Layer Implementation)
**Status**: Accepted
**Date**: 2026-01-10
**Author**: NDP Architect
**Supersedes**: None

---

## Context

The Silver ETL engine (duckdb-rs, per ADR-006-001) must be deployed somehow. Two primary options exist:

1. **Integrated**: Add Silver ETL as a module within the existing `air-quality-app` binary
2. **Separate**: Create a new `silver-etl` binary in `apps/silver-etl/`

This decision has implications for:
- Process isolation and failure domains
- Resource management and scheduling
- Development velocity and testing
- Deployment complexity

### Existing Architecture

The current production deployment includes:
- `air-quality-app`: Rust binary handling Bronze ingestion (Sources -> Channel -> ParquetStore)
- Memory limit: 512MB for air-quality-app
- Critical responsibility: Bronze data capture must not be interrupted

### Governing Principle

From `arch-data-lake-layers` pattern:
> "Bronze must succeed. Silver is best-effort and can be rebuilt from Bronze."

This principle directly informs the architectural decision.

---

## Decision

**Create a separate `silver-etl` binary** in `apps/silver-etl/`.

```
apps/
  air-quality-app/    # Bronze ingestion (existing)
  silver-etl/         # Silver ETL (new)
    Cargo.toml
    src/
      main.rs
      config.rs
      etl.rs
      sql_gen.rs
```

The separate binary:
- Runs as an independent process
- Scheduled via systemd timer (hourly)
- Has its own memory budget (~200MB)
- Can fail without impacting Bronze reliability

---

## Consequences

### Positive

1. **Process isolation** - Silver failures cannot crash Bronze ingestion
2. **Independent scheduling** - ETL runs on its own cadence (hourly timer)
3. **Resource isolation** - Separate memory limit prevents resource contention
4. **Simpler testing** - ETL logic can be tested independently
5. **Clear boundaries** - Separation of concerns enforced architecturally
6. **Operational flexibility** - Can restart, upgrade, or debug Silver without Bronze impact
7. **Future integration path** - Can merge later if stability is proven

### Negative

1. **Additional artifact** - One more binary to build and deploy
2. **Configuration duplication** - Both binaries need etcd connection
3. **Docker Compose complexity** - Additional service definition
4. **Monitoring surface** - Two services to monitor instead of one

### Neutral

1. **Config-client reuse** - Both binaries use same config-client crate
2. **Shared core crate** - Common types in `core/` crate
3. **Separate versioning** - Can evolve independently

---

## Alternatives Considered

### Alternative: Integrated into air-quality-app

**Description**: Add Silver ETL as a module within air-quality-app, triggered by internal timer or message.

| Factor | Integrated | Separate |
|--------|------------|----------|
| Process isolation | No | Yes |
| Failure domain | Shared | Isolated |
| Memory budget | Combined | Independent |
| Scheduling | Internal timer | systemd timer |
| Testing | Integrated | Independent |
| Deployment | Single binary | Two binaries |

**Rejected because**:

1. **Bronze reliability risk**: ETL bugs could crash the main ingestion loop
2. **Resource contention**: ETL memory spike could trigger OOM during ingestion
3. **Scheduling complexity**: Internal timer adds state management to ingestion app
4. **Testing burden**: Integration testing required for every ETL change

### Counter-argument: Future Integration

The separate binary approach does not prevent future integration. Once Silver ETL is proven stable:
- Could merge as a feature-flagged module
- Could run ETL in a separate thread/task
- Could share process with appropriate isolation

**Starting separated is safer; integration can happen later if beneficial.**

---

## Architecture Diagram

```
┌─────────────────────────────────────┐
│         air-quality-app             │
│    (Bronze layer - CRITICAL)        │
│                                     │
│  Sources → Channel → ParquetStore   │
│                                     │
│  Memory: 512MB                      │
│  Uptime: Continuous                 │
└──────────────────┬──────────────────┘
                   │
                   │ writes Parquet files
                   ▼
┌─────────────────────────────────────┐
│ /data/raw/{stream-id}/              │
│   year=/month=/day=/data.parquet    │
│                                     │
│ (Bronze - Authoritative Archive)    │
└──────────────────┬──────────────────┘
                   │
                   │ reads Parquet (hourly)
                   ▼
┌─────────────────────────────────────┐
│      silver-etl (NEW BINARY)        │
│    (Silver layer - Best Effort)     │
│                                     │
│  ConfigLoader → DuckDB → TimescaleDB│
│                                     │
│  Memory: 256MB                      │
│  Schedule: Hourly (systemd timer)   │
└─────────────────────────────────────┘
                   │
                   │ writes to PostgreSQL
                   ▼
┌─────────────────────────────────────┐
│       TimescaleDB (Silver)          │
│                                     │
│  silver.air_quality_observations    │
│  silver.weather_observations        │
│  silver.weather_forecasts           │
│  silver.outdoor_air_quality         │
│                                     │
│  Memory: 256MB                      │
└─────────────────────────────────────┘
```

---

## Implementation Details

### Crate Structure

```toml
# apps/silver-etl/Cargo.toml
[package]
name = "silver-etl"
version = "0.1.0"
edition = "2021"

[dependencies]
neural-core = { path = "../../core" }
config-client = { path = "../../config-client" }
duckdb = { version = "1.1", features = ["bundled", "parquet", "json"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
tracing = "0.1"
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }
```

### Entry Point

```rust
// apps/silver-etl/src/main.rs
use clap::Parser;
use config_client::ConfigClient;
use silver_etl::{EtlRunner, Config};
use tracing::{info, error};

#[derive(Parser)]
#[command(name = "silver-etl")]
struct Args {
    /// Run for specific stream only
    #[arg(short, long)]
    stream: Option<String>,

    /// Dry run - generate SQL but don't execute
    #[arg(long)]
    dry_run: bool,

    /// Full backfill from beginning
    #[arg(long)]
    backfill: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::init();
    let args = Args::parse();

    info!("Silver ETL starting");

    let config_client = ConfigClient::from_env().await?;
    let runner = EtlRunner::new(config_client)?;

    let result = runner.run(args.stream, args.dry_run, args.backfill).await;

    match result {
        Ok(stats) => {
            info!(
                streams = stats.streams_processed,
                rows = stats.total_rows,
                duration_ms = stats.duration_ms,
                "Silver ETL completed"
            );
        }
        Err(e) => {
            error!(error = %e, "Silver ETL failed");
            std::process::exit(1);
        }
    }

    Ok(())
}
```

### Docker Compose Service

```yaml
# deploy/pi/docker-compose.yml (addition)
services:
  silver-etl:
    build:
      context: ../..
      dockerfile: deploy/pi/Dockerfile.silver-etl
    image: ndp-silver-etl:latest
    container_name: ndp-silver-etl
    deploy:
      resources:
        limits:
          memory: 256M
    environment:
      - NDP_ETCD_ENDPOINTS=http://etcd:2379
      - NDP_TIMESCALE_HOST=timescaledb
      - NDP_TIMESCALE_PORT=5432
      - NDP_TIMESCALE_DB=ndp
      - NDP_RAW_PATH=/data/raw
      - RUST_LOG=info
    volumes:
      - air-quality-data:/data:ro  # Read-only access to Bronze
    depends_on:
      etcd:
        condition: service_healthy
      timescaledb:
        condition: service_healthy
    networks:
      - neural-network
    # Note: This container exits after ETL run
    # Scheduled by external systemd timer
    profiles:
      - etl  # Only start when explicitly requested
```

### Systemd Timer

```ini
# deploy/pi/systemd/silver-etl.timer
[Unit]
Description=NDP Silver ETL Timer
Requires=docker.service
After=docker.service

[Timer]
OnCalendar=*:05:00
Persistent=true
RandomizedDelaySec=60

[Install]
WantedBy=timers.target
```

```ini
# deploy/pi/systemd/silver-etl.service
[Unit]
Description=NDP Silver ETL Run
Requires=docker.service
After=docker.service

[Service]
Type=oneshot
ExecStart=/usr/bin/docker compose -f /opt/ndp/deploy/pi/docker-compose.yml --profile etl run --rm silver-etl
StandardOutput=journal
StandardError=journal
TimeoutStartSec=300

[Install]
WantedBy=multi-user.target
```

---

## Memory Budget

| Service | Current | With Silver ETL |
|---------|---------|-----------------|
| mosquitto | 128MB | 128MB |
| etcd | 256MB | 256MB |
| air-quality-app | 512MB | 512MB |
| TimescaleDB | 256MB | 256MB |
| Grafana | 256MB | 256MB |
| **silver-etl** | - | **256MB** |
| **Total** | 1408MB | **1664MB** |

Pi 5 16GB: 1664MB / 16000MB = **10.4% utilization** (comfortable margin)

---

## References

1. Pattern: `arch-data-lake-layers` - "Bronze must succeed, Silver is best-effort"
2. Pattern: `arch-domain-adapter-pattern` - Hexagonal architecture
3. Research: `research/agenticdataplatform/silver/06-refined-synthesis.md`
4. ADR-006-001: ETL Engine Selection (duckdb-rs)

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Architect | Initial decision |
