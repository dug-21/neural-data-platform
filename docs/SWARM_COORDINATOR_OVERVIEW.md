# Neural Data Platform - Swarm Coordinator Overview

**Generated**: 2025-12-15
**Coordinator**: SwarmLead
**Location**: /workspaces/neural-data-platform
**Status**: Active Coordination

---

## Executive Summary

The Neural Data Platform is a sophisticated Rust-based system combining neural trading capabilities with IoT data processing, specifically focused on air quality monitoring. The platform demonstrates production-grade architecture with microservices, distributed configuration, and real-time data pipelines.

**Current State**:
- 10 workspace members (core services + apps)
- Air Quality App in active development (AIR-001 through AIR-004 features)
- Configuration infrastructure complete (etcd + config-store)
- Production-ready core components requiring integration
- Recent DevOps fixes for data persistence

---

## System Architecture

### Platform Components Hierarchy

```
neural-data-platform/
├── Core Infrastructure
│   ├── core/ (platform-core)           - Shared utilities, traits, abstractions
│   ├── neural-core/                    - Neural network core library
│   └── config-store/                   - Distributed configuration service
│
├── Domain Services
│   ├── domains/air-quality/            - Air quality domain models & parsers
│   ├── neural-trading/                 - Trading engine
│   ├── neural-ml-ops/                  - ML operations
│   └── data-staging/                   - Data pipeline staging
│
├── Applications
│   └── apps/air-quality-app/           - Air quality REST API + MQTT ingestion
│
├── Supporting Services
│   ├── config-client/                  - Config client library
│   └── mcp-trading-server/             - MCP integration server
│
└── Vendor
    └── vendor/ruv-fann/                - Neural network & swarm libraries
        ├── neuro-divergent/
        └── ruv-swarm/
```

---

## Active Feature Development

### AIR-001: Configuration Infrastructure (COMPLETE)
**Status**: ✅ DevOps Complete - Ready for Testing
**Branch**: feature/air-001-implementation
**Focus**: Configuration management and data persistence

**Accomplishments**:
- Fixed 3 critical configuration inconsistencies
- Resolved Docker volume mount vs storage path mismatch
- Corrected environment variable naming (DATA_DIR → STORAGE_PATH)
- Unified all config sources to use /app/data
- etcd integration implemented (pending activation in main.rs)

**Critical Fixes Applied**:
1. docker-compose.yml: Environment variable name corrected
2. Base config: Storage path aligned with volume mount
3. Production config: Path standardized to /app/data

**Configuration Flow**:
```
Docker Volume: /app/data
     ↓
Environment: STORAGE_PATH=/app/data
     ↓
etcd (future) → YAML config → Defaults
     ↓
Application: Consistent /app/data path
```

**Pending Work**:
- Developer: Integrate etcd config loading in main.rs
- Tester: Validate data persistence scenarios
- Production: Sync configs to etcd after testing

**Risk Assessment**:
- Before: HIGH data loss risk, NOT production ready
- After: LOW risk, READY with YAML config
- After etcd integration: MINIMAL risk, FULLY automated

---

### AIR-002: MQTT Ingestion Pipeline (IN PROGRESS)
**Status**: 🔄 Architecture Complete - Implementation Needed
**Focus**: End-to-end data flow from MQTT to storage

**Architecture Summary**:
```
AirGradient Sensor → MQTT Broker → MqttSource → Parser →
    Validator → Adapter → ParquetStore (WAL) → REST API
```

**Core Design Principles**:
1. **REUSE OVER REWRITE**: Leverage existing platform components
2. **Minimal Configuration**: YAML + env vars for AIR-002
3. **Production Patterns**: Batching, DLQ, retry logic, WAL

**Production-Ready Components** ✅:
- MQTT Source (core/src/sources/mqtt.rs) - 478 lines tested
- AirGradient Parser (domains/air-quality/src/parser.rs) - 329 lines
- Parquet Storage (core/src/storage/parquet.rs) - 286 lines
- Write-Ahead Log (core/src/storage/wal.rs) - 172 lines

**Critical Blockers** 🚨:
1. **Main Application Uses Mock Services**
   - File: apps/air-quality-app/src/main.rs (lines 34-162)
   - Issue: Production code uses create_mock_services()
   - Impact: No actual data storage or MQTT processing
   - Fix Time: 4-8 hours

2. **MCP Server Placeholder Implementations**
   - File: apps/air-quality-app/src/mcp/server.rs (lines 17-87)
   - Issue: Returns fake hardcoded data
   - Impact: MCP tools non-functional
   - Fix Time: 2-4 hours

**Configuration Strategy**:
- **Now (AIR-002)**: Simple YAML config (1-2 hours)
- **Later (AIR-003)**: Full config-store client (3-4 weeks)
- **Timeline Savings**: 37% faster to E2E testing

**Key Design Decisions** (ADRs):
- ADR-001: Reuse core MqttSource (no domain wrapper)
- ADR-002: Dual-criteria batching (100 points OR 1 second)
- ADR-003: Dead Letter Queue for error handling
- ADR-004: Tokio + Arc/Mutex concurrency model
- ADR-005: YAML + environment overrides
- ADR-006: Defer config-store to AIR-003 for speed

**Performance Projections**:
- Single sensor: 1,440 readings/day (negligible load)
- 100 sensors: 144,000 points/day (~2 points/sec avg)
- Max latency: 1 second (batch timeout)
- Write throughput: Easily handles projected load

---

### AIR-003: Configuration Standardization (PLANNED)
**Status**: 📋 Planned
**Timeline**: 3-4 weeks
**Focus**: Platform-wide config-store adoption

**Scope**:
- Build config-store-client crate with:
  - Provider system (Env → gRPC → File)
  - LRU cache with TTL
  - Type-safe deserialization
  - Hot-reload capability via watch streams
- Migrate air-quality-app from YAML to config-store
- Remove duplicate AppConfig structs

**Architecture**:
```
┌─────────────────────────────────────┐
│         ConfigClient                │
├─────────────────────────────────────┤
│  Cache  │  Watcher  │  Metrics      │
│    ↓         ↓           ↓          │
│  ┌────────────────────────────┐    │
│  │   Provider System          │    │
│  ├──────┬──────┬──────────────┤    │
│  │ Env  │ File │ gRPC (etcd)  │    │
│  └──────┴──────┴──────────────┘    │
└─────────────────────────────────────┘
```

---

### AIR-004: Additional Features (FUTURE)
**Status**: 📋 Defined in feature directory
**Timeline**: TBD
**Focus**: Platform enhancements

---

## Core Technology Stack

### Runtime & Async
- **Rust**: 1.70+ (stable toolchain)
- **Tokio**: Async runtime with full features
- **Async-trait**: Trait-based async abstractions

### Data Processing
- **Polars**: DataFrame processing (0.35+)
- **Parquet**: Columnar storage format
- **Serde**: Serialization (JSON, YAML)

### Networking & Messaging
- **MQTT**: Eclipse Mosquitto client (rumqttc 0.24)
- **gRPC**: Tonic 0.12 + Prost 0.13
- **HTTP**: Axum 0.7 web framework

### Storage & Configuration
- **etcd**: Distributed key-value store (v3.5.11)
- **Redis**: Caching and streams (0.26)
- **TimescaleDB**: Time-series data (PostgreSQL extension)

### Observability
- **Tracing**: Structured logging (tracing + tracing-subscriber)
- **Prometheus**: Metrics collection
- **Grafana**: Dashboards and visualization

### Infrastructure
- **Docker**: Containerization (Compose 3.8)
- **Kubernetes**: Planned deployment target

---

## Project Structure Analysis

### Workspace Members (10 Total)

| Crate | Type | Purpose | Status |
|-------|------|---------|--------|
| **platform-core** (core/) | Library | Shared utilities, traits, MQTT source, Parquet storage | ✅ Production |
| **neural-core** | Library | Neural network core, math utilities | ✅ Production |
| **config-store** | Binary+Lib | gRPC config service, etcd integration | ✅ Production |
| **config-client** | Library | Config client (basic, needs enhancement) | 🔄 Enhancement needed |
| **air-quality** (domains/) | Library | Domain models, parser, validator, adapter | ✅ Production |
| **air-quality-app** (apps/) | Binary | REST API + MQTT ingestion app | 🔄 Integration needed |
| **neural-trading** | Binary+Lib | Trading engine | ✅ Production |
| **neural-ml-ops** | Service | ML operations | ✅ Production |
| **data-staging** | Service | Data pipeline staging | ✅ Production |
| **mcp-trading-server** | Binary | MCP integration | 🔄 Development |

### Key Directories

**Feature Development**: `/product/features/`
- air-001/ - Config infrastructure (COMPLETE)
- air-002/ - MQTT pipeline (IN PROGRESS)
- air-003/ - Config standardization (PLANNED)
- air-004/ - Future features
- config-store-tdd/ - Config store TDD
- v2Planning/ - Version 2 planning

**Documentation**: `/docs/`
- 140+ markdown files covering architecture, design, implementation

**Testing**: `/tests/`
- Component tests (config_store/)
- Emergency tests
- Integration validation

**Configuration**: `/config/`
- base/ - Base configurations
- overlays/ - Environment-specific overrides (development, production)
- Hierarchical configuration model

**Deployment**: `/deploy/`
- pi/ - Raspberry Pi deployment
- docker-compose configurations

**Contracts**: `/contracts/`
- gRPC proto definitions

---

## Docker Compose Infrastructure

### Development Stack (docker-compose.yml)

**Services**:
1. **mosquitto** (MQTT Broker)
   - Ports: 1883 (MQTT), 9001 (WebSocket)
   - Volume: Persistent data and logs
   - Health check: Active

2. **etcd** (Distributed Config)
   - Ports: 2379 (client), 2380 (peer)
   - Volume: Persistent data
   - Health check: Active

3. **air-quality-app**
   - Ports: 8080 (HTTP), 9090 (Metrics)
   - Volumes: Config files + persistent data
   - Environment: Full configuration via env vars
   - Dependencies: mosquitto, etcd
   - Health check: HTTP endpoint

4. **prometheus** (Metrics - Optional)
   - Port: 9091
   - Profile: monitoring

5. **grafana** (Dashboards - Optional)
   - Port: 3000
   - Profile: monitoring
   - Credentials: admin/admin

**Volumes**:
- mosquitto-data, mosquitto-logs
- air-quality-data (critical for persistence)
- air-quality-models
- etcd-data
- prometheus-data, grafana-data

**Network**: neural-network (bridge mode)

---

## Configuration Management Architecture

### Current Hierarchy (Post AIR-001 Fixes)

```
Priority 1: Environment Variables (highest)
    ↓
Priority 2: config.yaml file
    ↓
Priority 3: Hardcoded defaults (fallback)
```

### Supported Environment Variables

**MQTT Configuration**:
- `MQTT_BROKER_URL` - Broker hostname
- `MQTT_PORT` - Broker port
- `MQTT_CLIENT_ID` - Client identifier
- `MQTT_TOPIC_PATTERN` - Topic subscription pattern

**Storage Configuration**:
- `STORAGE_PATH` - Parquet storage base path (critical!)

**Server Configuration**:
- `SERVER_HOST` - HTTP bind address
- `SERVER_PORT` - HTTP port

**Other**:
- `RUST_LOG` - Logging level (debug, info, warn, error)
- `ETCD_ENDPOINTS` - etcd connection string

### etcd Configuration Hierarchy (Implemented)

```
/neural-data-platform/
├── apps/
│   └── air-quality/
│       ├── mqtt/broker_url
│       ├── mqtt/port
│       ├── storage/base_path
│       └── storage/wal_enabled
├── environments/
│   ├── production/
│   ├── staging/
│   └── development/
└── global/
    ├── logging/level
    └── monitoring/enabled
```

---

## Critical Path Issues & Solutions

### Issue 1: Data Persistence (RESOLVED ✅)
**Problem**: Multiple config sources pointed to different paths
**Solution**: Unified all sources to /app/data
**Status**: Fixed in AIR-001
**Impact**: Production-ready data persistence

### Issue 2: Mock Services in Production (CRITICAL 🚨)
**Problem**: main.rs uses create_mock_services()
**Location**: apps/air-quality-app/src/main.rs (lines 34-162)
**Solution**: Wire real MqttSource, ParquetStore implementations
**Effort**: 4-8 hours
**Impact**: Blocks all real functionality
**Priority**: P0 - Must fix immediately

### Issue 3: Config Store Client Missing
**Problem**: No reusable config-store client crate
**Solution**: Build in AIR-003 (3-4 weeks)
**Status**: Deferred for pragmatic timeline
**Impact**: Manual config management for now

### Issue 4: etcd Integration Not Activated
**Problem**: Config manager exists but not used in main.rs
**Solution**: Add etcd config loading with YAML fallback
**Effort**: 2-3 hours
**Priority**: P1 - Important for production automation

---

## Development Workflow (SPARC Methodology)

### SPARC Phases
1. **Specification** - Requirements analysis
2. **Pseudocode** - Algorithm design
3. **Architecture** - System design
4. **Refinement** - TDD implementation
5. **Completion** - Integration

### Available Tools
- `npx claude-flow sparc tdd "<feature>"` - Full TDD workflow
- `npx claude-flow sparc run <mode> "<task>"` - Specific mode
- `npx claude-flow sparc pipeline "<task>"` - Complete pipeline

### Git Workflow
- Main branch: `main`
- Feature branches: `feature/air-00X-*`
- Commit with coordination: Use hooks for pre/post operations

---

## Swarm Agent Coordination Plan

### Phase 1: Immediate Actions (Today)

**Priority P0 - Critical Blockers**:

1. **Researcher Agent**
   - Task: Analyze main.rs mock service pattern
   - Deliverable: Detailed integration plan
   - Duration: 1-2 hours

2. **Architect Agent**
   - Task: Design component wiring strategy
   - Deliverable: Integration architecture diagram
   - Duration: 2-3 hours

3. **Coder Agent**
   - Task: Replace mock services with real implementations
   - Files: apps/air-quality-app/src/main.rs
   - Duration: 4-8 hours
   - Dependencies: Architect's design

4. **Tester Agent**
   - Task: Validate AIR-001 data persistence fixes
   - Checklist: product/features/air-001/testing-checklist.md
   - Duration: 3-4 hours
   - Dependencies: None (can run in parallel)

### Phase 2: Integration & Testing (Days 2-3)

**Priority P1 - Important**:

5. **DevOps Agent**
   - Task: Verify Docker deployment configuration
   - Deliverable: Deployment validation report
   - Duration: 2-3 hours

6. **Coder Agent**
   - Task: Activate etcd config loading in main.rs
   - Deliverable: Hot-reload config integration
   - Duration: 2-3 hours

7. **Tester Agent**
   - Task: End-to-end integration testing
   - Scope: MQTT → Storage → API flow
   - Duration: 4-6 hours

### Phase 3: Production Readiness (Week 2)

**Priority P2 - Enhancement**:

8. **Architect Agent**
   - Task: Design config-store-client crate (AIR-003)
   - Deliverable: Architecture specification
   - Duration: 1 week

9. **Coder Agent (Team)**
   - Task: Implement config-store-client
   - Duration: 2-3 weeks
   - Team size: 2-3 agents

10. **Documentation Agent**
    - Task: Update platform documentation
    - Deliverable: API docs, deployment guides
    - Duration: 1 week

---

## Coordination Protocols

### Daily Standup Pattern
**Frequency**: Every 6 hours
**Format**: Status updates via memory system
**Topics**:
- Tasks completed since last sync
- Current work in progress
- Blockers requiring escalation
- Next 6-hour objectives

### Memory Coordination
**Namespace**: `swarm/neural-data-platform/`
**Keys**:
- `coordination/status` - Overall swarm status
- `tasks/<agent-id>/<task-id>` - Individual task state
- `blockers/<blocker-id>` - Escalated issues
- `metrics/performance` - Agent performance data

### Task Assignment Algorithm
```rust
fn assign_task(task: Task, agents: Vec<Agent>) -> Agent {
    // 1. Filter by capability match
    let capable = agents.filter(|a| a.has_capability(task.required));

    // 2. Score by specialization and performance
    let scored = capable.score_by_specialization(task.domain);

    // 3. Check workload balance
    let balanced = scored.sort_by_workload();

    // 4. Return best match
    balanced.first()
}
```

### Escalation Thresholds
- **Performance**: <70% success rate or >2x expected duration
- **Resource**: >90% agent utilization
- **Quality**: Failed quality gates or compliance violations

---

## Success Metrics

### Technical Metrics
- **Build Success**: 100% workspace members compile
- **Test Coverage**: >80% for critical paths
- **Integration Tests**: All passing
- **Docker Health**: All services healthy

### Feature Completion Metrics
- **AIR-001**: DevOps complete, testing in progress
- **AIR-002**: 60% complete (components ready, integration needed)
- **AIR-003**: 0% (planning phase)

### Performance Metrics
- **Data Ingestion**: <1s latency from MQTT to storage
- **API Response**: <100ms for health check
- **Storage Write**: <500ms for batch (100 points)
- **System Uptime**: >99.5% target

---

## Risk Assessment

### High-Risk Areas 🔴
1. **Mock Services in Production** (P0)
   - Mitigation: Immediate replacement planned
2. **Config Store Client Missing** (P1)
   - Mitigation: Pragmatic YAML approach for AIR-002
3. **Limited Integration Testing** (P1)
   - Mitigation: Dedicated tester agent assigned

### Medium-Risk Areas 🟡
1. **etcd Not Activated** (P1)
   - Mitigation: Can use YAML fallback temporarily
2. **Single App Coverage** (P2)
   - Mitigation: Expand to other apps in AIR-003+

### Low-Risk Areas 🟢
1. **Core Components Stable** ✅
2. **Docker Infrastructure Working** ✅
3. **Configuration Consistency Fixed** ✅

---

## Next Immediate Actions

### For SwarmLead (This Agent)
1. ✅ Complete project overview (this document)
2. 🔄 Spawn specialized agents for Phase 1
3. 🔄 Create TodoWrite tracking for all tasks
4. 🔄 Initialize memory coordination namespace

### For Team
1. **Researcher**: Analyze mock service replacement approach
2. **Architect**: Design component wiring strategy
3. **Coder**: Begin main.rs integration (after design)
4. **Tester**: Start AIR-001 validation testing

### Timeline Projection
- **Today**: Phase 1 coordination and analysis
- **Day 2-3**: Integration implementation and testing
- **Week 1**: AIR-001 and AIR-002 completion
- **Week 2-4**: AIR-003 config-store client development

---

## Contact & Resources

### Key Files for New Agents
- Project overview: `/workspaces/neural-data-platform/README.md`
- Architecture: `/workspaces/neural-data-platform/docs/`
- Features: `/workspaces/neural-data-platform/product/features/`
- Core code: `/workspaces/neural-data-platform/core/src/`
- Air quality app: `/workspaces/neural-data-platform/apps/air-quality-app/`

### Development Environment
- **Location**: /workspaces/neural-data-platform
- **Git Branch**: main (feature branches for work)
- **Docker**: Required for testing
- **Rust**: 1.70+ toolchain

### Communication Channels
- **Memory System**: swarm/neural-data-platform/*
- **Documentation**: Update in /docs/
- **Feature Tracking**: /product/features/

---

**Document Status**: ✅ Complete
**Last Updated**: 2025-12-15
**Coordinator**: SwarmLead
**Next Review**: Daily (after each phase completion)

---

## Appendix: Quick Reference Commands

### Build & Test
```bash
# Build entire workspace
cargo build --workspace

# Build specific app
cargo build -p air-quality-app

# Run tests
cargo test --workspace

# Run specific app
cargo run -p air-quality-app
```

### Docker Operations
```bash
# Start development stack
cd /workspaces/neural-data-platform
docker compose up -d

# Check service health
docker compose ps

# View logs
docker compose logs -f air-quality-app

# Stop stack
docker compose down
```

### Configuration
```bash
# Sync config to etcd
./scripts/sync-config-to-etcd.sh development

# View etcd config
docker exec etcd etcdctl get --prefix /air-quality/

# Test config loading
cargo test -p air-quality-app config_
```

### SPARC Workflow
```bash
# Run TDD workflow
npx claude-flow sparc tdd "MQTT ingestion pipeline"

# Run specific phase
npx claude-flow sparc run architect "Component integration"

# Full pipeline
npx claude-flow sparc pipeline "AIR-002 integration"
```

---

**End of Coordinator Overview**
