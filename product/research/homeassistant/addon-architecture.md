# Home Assistant Addon Architecture Analysis

**Date**: 2025-12-30
**Author**: NDP Research Agent
**Status**: Complete
**Related**: AIR-008, DP-001

---

## Executive Summary

This document analyzes whether NDP should be packaged as a Home Assistant addon or remain a separate platform that integrates with HA. After thorough research, the **recommended approach is to keep NDP as a separate platform** with HA integration via WebSocket/MQTT, rather than packaging it as an HA addon.

**Key Finding**: HA addons are designed for single-container applications. NDP's multi-container architecture (5 services) does not fit the addon model well. Additionally, NDP provides capabilities beyond what HA's ecosystem targets (ML/analytics vs. home automation visualization).

---

## 1. Home Assistant Addon Architecture Overview

### 1.1 What is an HA Addon?

A Home Assistant addon is a Docker container managed by the HA Supervisor. Key characteristics:

- **Single Container**: Each addon runs as one Docker container
- **S6 Overlay**: Uses [S6 overlay](https://developers.home-assistant.io/blog/2022/05/12/s6-overlay-base-images/) for process supervision within the container
- **Config.yaml Manifest**: Defines ports, volumes, permissions, and resource limits
- **Alpine-Based**: Built on Alpine Linux base images for minimal footprint
- **Supervisor Integration**: Managed by HA Supervisor (start/stop/update/logs)

### 1.2 Addon File Structure

```
addon-repository/
  addon-name/
    config.yaml          # Addon manifest
    build.yaml           # Multi-arch build config
    Dockerfile           # Container definition
    rootfs/              # Files to copy into container
      etc/
        s6-overlay/
          s6-rc.d/       # S6 service definitions
            service-name/
              run        # Service start script
              finish     # Service cleanup script
    DOCS.md              # Documentation
    CHANGELOG.md
    icon.png
    logo.png
```

### 1.3 Config.yaml Key Options

Based on [HA Developer Docs](https://developers.home-assistant.io/docs/add-ons/configuration/):

```yaml
name: "Example Addon"
version: "1.0.0"
slug: example_addon
description: "Example addon description"
arch:
  - aarch64
  - amd64
  - armhf
  - armv7
  - i386

# Networking
ports:
  8080/tcp: 8080    # Container:Host port mapping
  1883/tcp: null    # Optional port (user configures)
host_network: false  # Use host networking (not recommended)
ingress: true       # Enable ingress (web UI through HA)
ingress_port: 8099  # Port for ingress

# Storage
map:
  - config:rw       # Map HA config directory
  - ssl:ro          # Map SSL certificates
  - share:rw        # Shared storage
  - data:rw         # Addon data directory

# Resources
timeout: 60         # Container startup timeout
init: false         # Use addon's own init (not Docker's)
tmpfs: true         # Mount /tmp as tmpfs

# Security
privileged: false   # Run as privileged (avoid)
full_access: false  # Full HA API access
```

### 1.4 S6 Overlay for Multi-Service

S6 overlay allows running multiple processes within a single container:

```bash
# rootfs/etc/s6-overlay/s6-rc.d/mosquitto/run
#!/command/with-contenv bashio

exec mosquitto -c /etc/mosquitto/mosquitto.conf

# rootfs/etc/s6-overlay/s6-rc.d/mosquitto/finish
#!/command/execlineb -S0

# Cleanup on exit
```

However, this is designed for **supporting services**, not multiple primary applications.

---

## 2. NDP Current Architecture

### 2.1 Docker Compose Services (5 Containers)

From `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`:

| Service | Image | Purpose | Memory Limit | Ports |
|---------|-------|---------|--------------|-------|
| `mosquitto` | eclipse-mosquitto:2.0 | MQTT broker for sensor data | 128MB | 1883, 9001 |
| `etcd` | quay.io/coreos/etcd:v3.5.11 | Configuration store | 256MB | 2379 |
| `air-quality-app` | Custom Rust app | Multi-stream ingestion | 512MB | 8080 |
| `duckdb` | datacatering/duckdb:v1.1.3 | Silver layer analytics | 512MB | - |
| `grafana` | grafana/grafana:latest-ubuntu | Data visualization | 256MB | 3000 |

**Total Memory**: ~1.7GB allocated (all services)

### 2.2 Inter-Service Dependencies

```
mosquitto (MQTT)
    |
    +---> air-quality-app (ingestion)
              |
              +---> Parquet files (Bronze)
                        |
                        +---> duckdb (Silver views)
                                  |
                                  +---> grafana (dashboards)

etcd (config) <---> air-quality-app
```

### 2.3 Unique NDP Characteristics

- **Rust-based core**: Custom binary with domain adapter pattern
- **Multi-source ingestion**: MQTT + HTTP polling + future HA WebSocket
- **Medallion architecture**: Bronze (Parquet) -> Silver (DuckDB) -> Gold (planned TimescaleDB)
- **ML-focused**: Designed for feature engineering and predictions
- **Edge-optimized**: Runs on Raspberry Pi 5 with resource constraints

---

## 3. Feasibility Analysis: NDP as HA Addon

### 3.1 Can an HA Addon Run Multiple Containers?

**No.** HA addons are strictly single-container. The Supervisor manages each addon as one Docker container.

**Workarounds attempted by community**:
1. [Sysbox-based Docker-in-Docker](https://community.home-assistant.io/t/encapsulating-supervisor-in-a-single-container-via-sysbox/584961) - "runs but isn't happy about it"
2. S6 overlay multiple services - Works for supporting processes, not primary applications

### 3.2 Could NDP Services Run in One Container via S6?

Theoretically possible but problematic:

| Service | Can Run in S6? | Issues |
|---------|----------------|--------|
| Mosquitto | Yes | Works well as S6 service |
| etcd | Possibly | Memory overhead, startup complexity |
| air-quality-app | Yes | Main application |
| DuckDB | Possibly | Needs shared volume access |
| Grafana | Yes | Resource-heavy, long startup |

**Problems**:
- **Memory**: Combined services exceed 1.7GB - HA addons should be lighter
- **Complexity**: S6 isn't designed for 5 independent databases/services
- **Resource contention**: No container isolation means one service affects others
- **Upgrades**: Updating Grafana requires rebuilding entire addon
- **Debugging**: Logs from 5 services in one container are difficult to parse

### 3.3 Alternative: Split into Multiple Addons

Could create separate addons:
- `ndp-mosquitto` (or use existing HA Mosquitto addon)
- `ndp-core` (air-quality-app + etcd)
- `ndp-analytics` (DuckDB + Grafana)

**Problems**:
- **Inter-addon communication**: Addons are isolated; networking requires host mode or complex config
- **Shared storage**: Difficult to share Parquet files between addons
- **Coordination**: No native way to ensure addons start in order
- **Maintenance**: 3x the addon maintenance burden

### 3.4 Comparison with Existing Complex HA Addons

**InfluxDB Addon** ([hassio-addons/addon-influxdb](https://github.com/hassio-addons/addon-influxdb)):
- Runs InfluxDB + Chronograf + Kapacitor (3 services)
- All are InfluxData products designed to work together
- Uses S6 for service management
- Still considered complex for an addon

**TimescaleDB Addon** ([Expaso/hassos-addon-timescaledb](https://github.com/Expaso/hassos-addon-timescaledb)):
- Runs PostgreSQL with TimescaleDB, PostGIS, pgVector extensions
- Single PostgreSQL instance with extensions (not separate services)
- Works because it's fundamentally one database process

**NDP Difference**: NDP has 5 truly independent services (different technologies, different resource profiles). This doesn't map well to the addon model.

---

## 4. Benefits and Limitations of HA Addons

### 4.1 Benefits of HA Addons

| Benefit | Description | NDP Relevance |
|---------|-------------|---------------|
| **Easy installation** | One-click install from HA UI | Good for non-technical users |
| **Managed lifecycle** | HA handles start/stop/restart | Reduces operational burden |
| **Integrated logs** | Logs visible in HA UI | Easier debugging |
| **Auto-updates** | Version management through HA | Simplified maintenance |
| **Backup integration** | Included in HA backups | Data protection |
| **Config UI** | Web-based configuration | No SSH/terminal needed |

### 4.2 Limitations of HA Addons

| Limitation | Description | NDP Impact |
|------------|-------------|------------|
| **Single container** | One Docker container per addon | Major - NDP has 5 services |
| **HA OS required** | Addons only work with HA OS/Supervised | Limits deployment options |
| **Resource isolation** | Addons share resources with HA | Risk of impacting HA performance |
| **Limited networking** | Complex network topologies difficult | Issue for multi-service communication |
| **Update dependencies** | Addon updates tied to HA ecosystem | May lag behind upstream |
| **No orchestration** | No docker-compose equivalent | Can't define service relationships |

### 4.3 Addon Audience

HA addons are ideal for:
- Single-purpose applications (file editor, SSH server, Samba)
- Applications that integrate directly with HA (Zigbee2MQTT, Node-RED)
- Services that HA users commonly need alongside HA

HA addons are NOT ideal for:
- Multi-tier applications requiring multiple databases
- Resource-intensive analytics/ML platforms
- Systems with complex service orchestration

---

## 5. Comparison: Addon vs. Separate Platform

### 5.1 Architecture Comparison

```
OPTION A: NDP as HA Addon (NOT RECOMMENDED)
==========================================
Home Assistant OS
  +-- HA Supervisor
        +-- HA Core
        +-- Mosquitto Addon (existing)
        +-- NDP Addon (single container with all services?)
              +-- air-quality-app
              +-- etcd
              +-- duckdb
              +-- grafana

Problems: Memory contention, no isolation, complex S6 config

OPTION B: NDP as Separate Platform (RECOMMENDED)
================================================
Home Assistant OS              Raspberry Pi 5 (same device)
  +-- HA Supervisor            +-- Docker Compose
        +-- HA Core                  +-- mosquitto
        +-- (no NDP addon)           +-- etcd
                                     +-- air-quality-app
Network Bridge <----------------->   +-- duckdb
(MQTT, WebSocket)                    +-- grafana

Benefits: Full isolation, proper orchestration, independent scaling
```

### 5.2 Feature Comparison Matrix

| Feature | NDP as Addon | NDP as Separate Platform |
|---------|--------------|-------------------------|
| **Installation complexity** | Medium (addon config) | Medium (docker-compose) |
| **Resource isolation** | Poor (shared container) | Excellent (separate containers) |
| **Service orchestration** | Limited (S6 only) | Full (docker-compose) |
| **Memory management** | Difficult (combined) | Precise (per-container limits) |
| **Independent updates** | Difficult | Easy |
| **Debugging** | Harder (combined logs) | Easier (per-service logs) |
| **Portability** | HA OS only | Any Docker host |
| **HA integration** | Built-in | Via API/MQTT/WebSocket |
| **Backup/restore** | HA-managed | Manual or scripted |
| **Multi-host deployment** | Not possible | Supported |
| **ML workloads** | Constrained | Full flexibility |

### 5.3 Integration Patterns (Separate Platform)

**Pattern 1: MQTT Bridge**
```
HA Entity State --> HA MQTT Integration --> Mosquitto --> NDP
```
- HA publishes state changes to MQTT
- NDP subscribes and ingests

**Pattern 2: WebSocket API (Recommended for AIR-008)**
```
HA WebSocket API --> HomeAssistantSource (NDP) --> Bronze Layer
```
- NDP connects directly to HA WebSocket
- Real-time state streaming
- Full entity context (attributes, history)

**Pattern 3: REST API Polling**
```
NDP HTTP Poller --> HA REST API --> Transform --> Bronze Layer
```
- Periodic polling of HA entity states
- Lower latency requirements
- Simpler implementation

---

## 6. Recommended Approach

### 6.1 Recommendation: Separate Platform with HA Integration

**Keep NDP as a separate Docker Compose stack** on the same Raspberry Pi (or separate device), integrating with HA via:

1. **Primary**: WebSocket API for real-time home events (AIR-008 design)
2. **Secondary**: MQTT for sensor data (existing AirGradient flow)
3. **Future**: REST API for on-demand queries

### 6.2 Rationale

| Factor | Weight | Addon Score | Separate Score | Notes |
|--------|--------|-------------|----------------|-------|
| Architectural fit | High | 2/10 | 9/10 | Multi-container doesn't fit addon model |
| Resource management | High | 3/10 | 9/10 | Proper isolation critical for edge |
| Maintenance burden | Medium | 5/10 | 7/10 | docker-compose is well-understood |
| User experience | Medium | 8/10 | 6/10 | Addon has nicer UX, but works either way |
| Future extensibility | High | 3/10 | 9/10 | Addon limits growth |
| ML/Analytics support | High | 2/10 | 10/10 | Addon constraints hurt ML workloads |

**Weighted Score**: Addon = 3.5/10, Separate = 8.5/10

### 6.3 When Addon MIGHT Make Sense

Consider addon approach IF:
- NDP simplifies to 1-2 containers in future
- HA adds multi-container addon support
- Target audience is strictly non-technical HA users
- Resource requirements decrease significantly

### 6.4 Hybrid Approach (Future Consideration)

Could create a **lightweight HA integration component** that:
- Installs as HACS custom component (not addon)
- Configures HA -> NDP data forwarding
- Provides NDP status in HA UI
- Manages connection settings

This gives HA-native UX without addon architecture constraints.

---

## 7. Implementation Guidance

### 7.1 Current Approach (Already Designed)

The AIR-008 feature already uses the correct pattern:

```yaml
# config/base/streams/home-events/config.yaml
sources:
  - type: home_assistant
    websocket_url: "${HASS_WEBSOCKET_URL}"
    access_token: "${HASS_ACCESS_TOKEN}"
    entity_filters:
      - "binary_sensor.window_*"
      - "binary_sensor.door_*"
```

NDP's `HomeAssistantSource` connects to HA WebSocket, receives events, and ingests to Bronze layer.

### 7.2 Network Configuration

For same-device deployment:

```yaml
# docker-compose.yml
networks:
  default:
    name: neural-network
    driver: bridge

# HA connection
environment:
  - HASS_WEBSOCKET_URL=ws://host.docker.internal:8123/api/websocket
  # or use host.docker.internal, or HA's IP address
```

### 7.3 User Documentation

Provide clear setup guide:
1. Generate HA long-lived access token
2. Configure NDP environment variables
3. Test WebSocket connectivity
4. Verify data flow in Bronze layer

---

## 8. Conclusion

### Key Findings

1. **HA addons are single-container** - NDP's 5-service architecture doesn't fit
2. **S6 overlay is for supporting services** - Not primary multi-database applications
3. **Existing complex addons** (InfluxDB, TimescaleDB) work because they're single-technology stacks
4. **Separate platform provides** better isolation, orchestration, and extensibility
5. **Integration via WebSocket/MQTT** provides good HA connectivity without addon constraints

### Final Recommendation

**Do NOT package NDP as a Home Assistant addon.** Instead:

1. Maintain NDP as a Docker Compose stack
2. Integrate with HA via WebSocket API (AIR-008 design)
3. Consider HACS integration component for UX
4. Document clear setup instructions for HA users

This approach maximizes NDP's capabilities while maintaining good HA integration.

---

## References

### Home Assistant Documentation
- [Add-on Configuration](https://developers.home-assistant.io/docs/add-ons/configuration/)
- [S6-Overlay Base Images](https://developers.home-assistant.io/blog/2022/05/12/s6-overlay-base-images/)
- [WebSocket API](https://developers.home-assistant.io/docs/api/websocket)
- [REST API](https://developers.home-assistant.io/docs/api/rest/)

### Community Discussions
- [Addons vs Separate Docker Containers](https://community.home-assistant.io/t/thoughts-on-running-addons-vs-separate-docker-containers/718028)
- [Encapsulating Supervisor in Single Container](https://community.home-assistant.io/t/encapsulating-supervisor-in-a-single-container-via-sysbox/584961)
- [Multiple Addon Instances](https://community.home-assistant.io/t/official-support-for-running-multiple-instances-of-the-same-add-on-docker-container/697906)

### Example Complex Addons
- [InfluxDB Addon](https://github.com/hassio-addons/addon-influxdb)
- [TimescaleDB Addon](https://github.com/Expaso/hassos-addon-timescaledb)
- [Example Addon](https://github.com/hassio-addons/addon-example)

### NDP Documentation
- [AIR-008 Home Events Integration](/workspaces/neural-data-platform/product/research/dp-analysis/home-assistant-integration.md)
- [Database Comparison](/workspaces/neural-data-platform/product/research/homeassistant/database-comparison.md)
- [Docker Compose Setup](/workspaces/neural-data-platform/deploy/pi/docker-compose.yml)

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-30 | Initial analysis and recommendation |
