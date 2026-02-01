# Deployment and Sync Process Research

## Executive Summary

This document details the deployment and configuration synchronization process for the Neural Data Platform (NDP). The system uses a layered configuration approach with YAML files as the source of truth, etcd as the runtime configuration store, and Docker Compose for container orchestration.

---

## 1. Deployment Scripts Overview

### 1.1 Main Deployment Scripts

| Script | Location | Purpose |
|--------|----------|---------|
| `deploy.sh` | `/workspaces/neural-data-platform/deploy/pi/deploy.sh` | Primary deployment script for Pi production and integration environments |
| `deploy.sh` | `/workspaces/neural-data-platform/scripts/deploy.sh` | Legacy Neural Trader deployment script (not for NDP streams) |

### 1.2 deploy/pi/deploy.sh Commands

The main deployment script (`/workspaces/neural-data-platform/deploy/pi/deploy.sh`) supports the following commands:

#### Core Commands
| Command | Description |
|---------|-------------|
| `deploy` | Full deploy (build + start all services) |
| `start` | Start all services (no rebuild) |
| `stop` | Stop all services |
| `logs` | View logs (follows) |
| `status` | Check service health and URLs |
| `build` | Build Docker images only |

#### Update Commands
| Command | Description |
|---------|-------------|
| `update [--no-cache] [target]` | Pull latest from git and rebuild. Targets: app, mcp, silver, all |
| `refresh` | Pull latest configs only (no rebuild, restarts Grafana) |

#### Configuration Commands
| Command | Description |
|---------|-------------|
| `sync` | Sync configuration to etcd |
| `init-streams` | Initialize stream configurations in etcd |
| `list-streams` | List configured streams from etcd |
| `sync-dictionary` | Sync entity schemas to TimescaleDB data dictionary |
| `sync-dimensions` | Sync dimension tables from config/base/dimensions/ |

#### Silver ETL Commands
| Command | Description |
|---------|-------------|
| `silver-migrate` | Run Silver Layer TimescaleDB schema migrations |
| `silver-etl` | Run Silver ETL once (Bronze -> TimescaleDB) |
| `silver-daemon` | Start Silver ETL in daemon mode (continuous) |
| `silver-daemon-stop` | Stop Silver ETL daemon |
| `silver-daemon-logs` | View Silver ETL daemon logs (follows) |
| `silver-daemon-status` | Check Silver ETL daemon status |

---

## 2. Sync Command Deep Dive

### 2.1 `sync` Command

**Purpose**: Syncs configuration files from the repository to etcd.

**Implementation** (`deploy/pi/deploy.sh` lines 304-318):
```bash
sync_config() {
    log "Syncing configuration to etcd..."

    # Wait for etcd to be ready
    until dcx etcd etcdctl endpoint health >/dev/null 2>&1; do
        warn "Waiting for etcd to be ready..."
        sleep 2
    done

    # Run the sync script from the repo root
    if [ -f "$REPO_ROOT/scripts/sync-config-to-etcd.sh" ]; then
        ETCD_CONTAINER=$ETCD_CONTAINER "$REPO_ROOT/scripts/sync-config-to-etcd.sh" $ENV_NAME
    else
        warn "Config sync script not found, skipping"
    fi
}
```

**Sync Script** (`/workspaces/neural-data-platform/scripts/sync-config-to-etcd.sh`):
1. Flattens YAML configuration files into key-value pairs
2. Uploads each key-value to etcd with a service prefix
3. Handles both base configs and environment overlays
4. Special handling for `streams/` directory - creates `/streams/{stream_id}/...` keys

**etcd Key Structure for Streams**:
```
/streams/{stream_id}/
    ├── stream_id
    ├── description
    ├── version
    ├── enabled
    ├── retention_days
    ├── fields/...
    ├── sources/...
    ├── silver_etl/...
    └── ...
```

### 2.2 `sync-dictionary` Command

**Purpose**: Syncs entity schemas from stream configs to TimescaleDB `data_dictionary` schema.

**Implementation** (`sync_to_data_dictionary()` function, lines 347-805):
1. Reads all stream config.yaml files from `config/base/streams/*/`
2. Generates SQL for Bronze layer metadata (streams, entity_schemas, attributes)
3. Generates SQL for Silver layer metadata (silver_tables, silver_columns, silver_lineage, silver_dq_rules)
4. Uses UPSERT (ON CONFLICT DO UPDATE) for Silver tables since multiple streams can feed the same Silver table
5. Executes the generated SQL against TimescaleDB

**Tables Updated**:
- `data_dictionary.streams`
- `data_dictionary.entity_schemas`
- `data_dictionary.entity_schema_attributes`
- `data_dictionary.silver_tables`
- `data_dictionary.silver_columns`
- `data_dictionary.silver_lineage`
- `data_dictionary.silver_dq_rules`
- `data_dictionary.sync_status`

### 2.3 `sync-dimensions` Command

**Purpose**: Syncs dimension tables from `config/base/dimensions/` to Silver layer.

**Implementation** (`sync_dimensions()` function, lines 934-995):
1. Discovers dimension YAML configs in `config/base/dimensions/`
2. For each dimension:
   - Parses config to get target table/schema
   - Reads source CSV file
   - Imports data using SQL COPY (or `ndp dimension sync` CLI if available)
3. Tracks sync status in state file at `data/.dimension_state`

---

## 3. etcd Integration

### 3.1 etcd Configuration

**Docker Compose Configuration** (`deploy/pi/docker-compose.yml`):
```yaml
etcd:
  image: quay.io/coreos/etcd:v3.5.11
  container_name: etcd
  ports:
    - "2379:2379"
  environment:
    - ETCD_NAME=etcd0
    - ETCD_DATA_DIR=/etcd-data
    - ETCD_LISTEN_CLIENT_URLS=http://0.0.0.0:2379
    - ETCD_ADVERTISE_CLIENT_URLS=http://etcd:2379
    - ETCD_QUOTA_BACKEND_BYTES=536870912  # 512MB quota
  volumes:
    - etcd-data:/etcd-data
```

### 3.2 etcd Key Paths

| Prefix | Purpose |
|--------|---------|
| `/streams/{stream_id}/...` | Stream configurations (synced from YAML) |
| `/air-quality/streams/{stream_id}/...` | Legacy stream metadata (MQTT device mappings) |
| `/air-quality/multi_stream/...` | Global multi-stream settings |

### 3.3 Config Client Library

**Location**: `/workspaces/neural-data-platform/config-client/`

**Key Components**:
- `ConfigClient` - Low-level etcd client with type-safe get/set
- `StreamRegistry` - High-level stream config management with caching
- `WatchHandle` - Config change watching capability

**Usage in Application** (`apps/air-quality-app/src/config_etcd.rs`):
```rust
pub async fn load_from_etcd() -> Result<EtcdAppConfig, ...> {
    let client = ConfigClient::with_prefix(&[&etcd_endpoint], "/air-quality").await?;
    // Load server, mqtt, storage sections with env var overrides
}
```

### 3.4 What Happens on Sync Failure

1. **etcd Not Ready**: Script retries every 2 seconds until etcd health check passes
2. **Sync Script Missing**: Warning logged, continues without syncing
3. **Individual Key Failure**: No explicit error handling - continues with other keys
4. **YAML Parse Error**: Python parser fails silently, key not written

**Recommendation**: Add explicit error handling and retry logic for individual key sync failures.

---

## 4. Docker Compose Configuration

### 4.1 Compose Files

| File | Environment | Use Case |
|------|-------------|----------|
| `deploy/pi/docker-compose.yml` | Production | Pi deployment with all services |
| `docker-compose.integration.yml` | Integration | Local testing with full stack |
| `docker-compose.yml` | Development | Basic development setup |
| `docker-compose.prod.yml` | Pi Production | Alternate Pi config (legacy) |

### 4.2 air-quality-app Service Configuration

**Key Volume Mounts** (`deploy/pi/docker-compose.yml`):
```yaml
volumes:
  - air-quality-data:/data                      # Parquet data storage
  - ../../config/base/streams:/config/streams:ro     # Stream configs (read-only)
  - ../../config/base/processors:/config/processors:ro  # Processor configs
```

**Key Environment Variables**:
```yaml
environment:
  - DATA_DIR=/data
  - ETCD_ENDPOINT=http://etcd:2379
  - MQTT_BROKER_URL=mosquitto
  - MQTT_PORT=1883
  - STREAM_CONFIG_DIR=/config/streams
  - TIMESCALE_URL=postgresql://postgres:${POSTGRES_PASSWORD}@timescaledb:5432/ndp
```

### 4.3 Service Dependencies

```
air-quality-app
    ├── depends_on: mosquitto (healthy)
    ├── depends_on: etcd (healthy)
    └── depends_on: timescaledb (healthy)

silver-etl
    ├── depends_on: timescaledb (healthy)
    └── depends_on: etcd (healthy)
```

---

## 5. Restart/Reload Process

### 5.1 Configuration Loading Order (air-quality-app startup)

```
1. StreamRegistry (etcd /streams/air-quality)
       ↓ if fails
2. Legacy etcd (/air-quality/*)
       ↓ if fails
3. config.yaml file
       ↓ if fails
4. Default config with env overrides
```

### 5.2 On-Startup Config Sync (AIR-005)

**Location**: `apps/air-quality-app/src/main.rs` (lines 137-168)

```rust
// Sync YAML configs to etcd on every startup
if std::path::Path::new(&config_dir).exists() {
    let sync_service = ConfigSyncService::new(&config_dir);
    match registry.sync_all(&registry).await {
        Ok(count) => info!("Synced {} stream configs to etcd", count),
        Err(e) => warn!("Config sync failed: {}. Using existing etcd configs.", e),
    }
}
```

**Key Points**:
- App syncs YAML -> etcd on **every startup**
- Sync failures are non-fatal (app continues with existing etcd configs)
- This is ONE-WAY sync (YAML is source of truth)

### 5.3 Hot Reload Capability

**Current Status**: **No hot reload** for stream configurations.

**Watch Capability Exists** (`config-client/src/watch.rs`):
```rust
pub struct WatchHandle {
    cancel_tx: mpsc::Sender<()>,
}

impl WatchHandle {
    pub async fn new<F>(client: Client, prefix: &str, callback: F) -> Result<Self, ConfigError>
    where
        F: Fn(String, Option<serde_json::Value>) + Send + Sync + 'static
}
```

**However**: The watch capability is **not used** in the main application. Config is only read at startup.

**To Apply Config Changes**:
1. Modify YAML files
2. Restart air-quality-app container: `docker restart air-quality-app`

### 5.4 Service Initialization Order

```
1. Docker Compose starts services in dependency order
2. etcd becomes healthy
3. TimescaleDB becomes healthy (init-scripts run on first start)
4. mosquitto becomes healthy
5. air-quality-app starts:
   a. Connects to etcd
   b. Syncs YAML configs to etcd
   c. Loads config from etcd
   d. Replays WAL for crash recovery
   e. Starts multi-stream coordinator
   f. Starts HTTP server
```

---

## 6. Pi Deployment Specifics

### 6.1 Prerequisites

- Raspberry Pi 5 (16GB RAM recommended)
- Ubuntu 25.04 (or similar ARM64 Linux)
- Docker and Docker Compose installed
- Git installed

### 6.2 Quick Start

```bash
# Clone and deploy
git clone https://github.com/dug-21/neural-data-platform.git
cd neural-data-platform/deploy/pi
chmod +x deploy.sh
./deploy.sh
```

**First build takes 15-30 minutes** (Rust compilation).

### 6.3 Deploy vs Update

| `./deploy.sh deploy` | `./deploy.sh update` |
|---------------------|---------------------|
| Full build from scratch | Pulls git changes first |
| Starts all services | Resets to origin/main |
| Syncs config to etcd | Can target specific services |
| Initializes streams | Rebuilds with cache |

### 6.4 Manual Steps Required

**Initial Setup**:
1. Clone repository
2. Create `.env` file with secrets (POSTGRES_PASSWORD, API keys)
3. Run `./deploy.sh deploy`

**Adding a New Stream**:
1. Create `config/base/streams/{stream-id}/config.yaml`
2. Run `./deploy.sh refresh` or restart air-quality-app
3. Run `./deploy.sh sync-dictionary` to update data dictionary
4. Run `./deploy.sh silver-migrate` if Silver table needed

**Updating Stream Config**:
1. Modify YAML file
2. Restart app: `docker restart air-quality-app`
3. Or use `./deploy.sh refresh` (includes Grafana restart)

---

## 7. Exact Sequence to Deploy a New Stream

### 7.1 Create Stream Configuration

```bash
# Create stream directory and config
mkdir -p config/base/streams/new-stream
cat > config/base/streams/new-stream/config.yaml << 'EOF'
stream_id: "new-stream"
description: "My new data stream"
version: "1.0.0"
enabled: true
retention_days: 90

fields:
  - name: value
    type: float
    nullable: false

sources:
  - type: mqtt
    enabled: true
    broker_url: "mosquitto"
    topic_pattern: "sensors/new/#"
EOF
```

### 7.2 Deploy Configuration

```bash
cd deploy/pi

# Option A: Full refresh (restarts Grafana)
./deploy.sh refresh

# Option B: Just restart the app
docker restart air-quality-app

# Verify config is in etcd
docker exec etcd etcdctl get --prefix "/streams/new-stream" --keys-only
```

### 7.3 Add Silver Layer (if needed)

```bash
# Add silver_etl section to config.yaml, then:
./deploy.sh sync-dictionary   # Update data dictionary
./deploy.sh silver-migrate    # Create Silver tables
./deploy.sh silver-daemon     # Start continuous ETL
```

---

## 8. Failure Modes and Symptoms

### 8.1 etcd Connection Failure

**Symptoms**:
- App starts but uses default/fallback config
- Logs show: "Failed to connect to etcd" or "Config sync failed"

**Resolution**:
1. Check etcd is running: `docker ps | grep etcd`
2. Check etcd health: `docker exec etcd etcdctl endpoint health`
3. Check network: `docker exec air-quality-app ping etcd`

### 8.2 Sync Script Errors

**Symptoms**:
- Logs show: "Config sync script not found"
- etcd has stale configuration

**Resolution**:
1. Verify script exists: `ls -la scripts/sync-config-to-etcd.sh`
2. Check script has execute permission
3. Run manually: `./scripts/sync-config-to-etcd.sh development`

### 8.3 YAML Parse Errors

**Symptoms**:
- Stream not appearing in etcd
- Python errors in sync script output

**Resolution**:
1. Validate YAML: `python3 -c "import yaml; yaml.safe_load(open('config.yaml'))"`
2. Check for tabs (use spaces only)
3. Check for special characters in strings

### 8.4 TimescaleDB Schema Mismatch

**Symptoms**:
- Silver ETL fails with column errors
- Data dictionary out of sync

**Resolution**:
1. Run `./deploy.sh sync-dictionary`
2. Run `./deploy.sh silver-migrate`
3. Check schema: `docker exec pi5-timescaledb psql -U postgres -d ndp -c "\d+ silver.*"`

### 8.5 Container Restart Loop

**Symptoms**:
- Container keeps restarting
- Health checks failing

**Resolution**:
1. Check logs: `docker logs air-quality-app --tail 100`
2. Check resource limits: `docker stats`
3. Check dependencies are healthy

---

## 9. Automation Opportunities

### 9.1 Currently Manual Steps That Should Be Automated

| Step | Current State | Automation Opportunity |
|------|--------------|----------------------|
| Create `.env` file | Manual copy from `.env.example` | Template generation script |
| Add Silver layer | Multiple commands | Single `add-stream` command |
| Schema migration | Separate command | Auto-run on config change |
| Dictionary sync | Separate command | Auto-run on config change |
| Hot reload | Requires restart | Implement config watch + reload |

### 9.2 Recommended Improvements

1. **Config Validation on Commit**: Add pre-commit hook to validate YAML syntax
2. **Atomic Sync**: Transaction-based etcd sync with rollback on failure
3. **Auto-Migration**: Detect schema changes and auto-migrate
4. **Health Dashboard**: Grafana panel showing sync status
5. **Config Diff**: Show what will change before applying

---

## 10. Key File Locations

### Configuration Files
- `/workspaces/neural-data-platform/config/base/streams/*/config.yaml` - Stream definitions
- `/workspaces/neural-data-platform/config/base/dimensions/*.yaml` - Dimension tables
- `/workspaces/neural-data-platform/config/base/processors/*.yaml` - Processor configs

### Deployment Scripts
- `/workspaces/neural-data-platform/deploy/pi/deploy.sh` - Main deployment script
- `/workspaces/neural-data-platform/scripts/sync-config-to-etcd.sh` - etcd sync script
- `/workspaces/neural-data-platform/deploy/pi/configs/streams/init-streams.sh` - Stream initialization
- `/workspaces/neural-data-platform/deploy/pi/configs/streams/add-stream.sh` - Add stream utility

### Docker Compose Files
- `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml` - Pi production
- `/workspaces/neural-data-platform/docker-compose.integration.yml` - Integration testing
- `/workspaces/neural-data-platform/docker-compose.yml` - Development

### Database Init Scripts
- `/workspaces/neural-data-platform/deploy/pi/init-scripts/*.sql` - TimescaleDB initialization

### Source Code (Config Loading)
- `/workspaces/neural-data-platform/config-client/` - etcd client library
- `/workspaces/neural-data-platform/apps/air-quality-app/src/config_etcd.rs` - App config loading
- `/workspaces/neural-data-platform/apps/air-quality-app/src/config_sync/service.rs` - YAML sync service

---

## 11. Summary

The NDP deployment system follows a **YAML-first, etcd-runtime** pattern:

1. **Source of Truth**: YAML files in `config/base/streams/`
2. **Runtime Store**: etcd (synced from YAML on app startup)
3. **No Hot Reload**: Config changes require container restart
4. **Multiple Sync Targets**: etcd, data dictionary, dimension tables

**Key Commands for Stream Deployment**:
```bash
# Initial deployment
./deploy.sh deploy

# Add/modify stream
# 1. Edit config/base/streams/{id}/config.yaml
# 2. Restart: docker restart air-quality-app
# 3. Update dictionary: ./deploy.sh sync-dictionary
# 4. Migrate Silver: ./deploy.sh silver-migrate
```

**Critical Gap**: No hot reload capability means all config changes require restart.
