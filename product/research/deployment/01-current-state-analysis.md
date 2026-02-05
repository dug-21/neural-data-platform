# Deploy.sh Current State Analysis

**Date**: 2026-02-05
**Analyst**: NDP Architect
**Subject**: Comprehensive analysis of `/deploy/pi/deploy.sh` (2,868 lines)

---

## Executive Summary

The `deploy.sh` script is the central deployment orchestrator for the Neural Data Platform. It has evolved organically through multiple feature additions (dp-013, dp-018, dp-020, fe-001, fe-002, v11-002, v11-007) into a 2,868-line monolithic bash script that handles everything from Docker orchestration to database schema generation.

**Key Findings**:
- 11-phase declarative deployment system implemented in bash
- Significant code duplication in YAML parsing functions (duplicated in ddl-generator.sh)
- Complex SQL generation via string concatenation (error-prone)
- Good idempotency patterns but fragile due to bash limitations
- External tool delegation emerging (ndp-gold-ddl, ndp-validate) as complexity grows

---

## 1. Function Inventory

### 1.1 Core Infrastructure Functions

| Function | Lines | Description | Complexity |
|----------|-------|-------------|------------|
| `dc()` | 86-88 | Docker compose wrapper | Low |
| `dcx()` | 92-94 | Docker compose exec wrapper | Low |
| `log()` | 102 | Colored logging (green) | Low |
| `warn()` | 103 | Colored warning (yellow) | Low |
| `error()` | 104 | Colored error + exit | Low |
| `check_prereqs()` | 328-333 | Docker prerequisite check | Low |
| `wait_for_health()` | 1370-1388 | Service health polling | Medium |

### 1.2 YAML Helper Functions (Lines 118-322)

| Function | Lines | Description | Code Smell |
|----------|-------|-------------|------------|
| `yaml_get()` | 120-178 | Extract YAML value | **DUPLICATED** in ddl-generator.sh |
| `yaml_array_len()` | 181-230 | Get array length | **DUPLICATED** in ddl-generator.sh |
| `yaml_array_get()` | 233-290 | Get array item | **DUPLICATED** in ddl-generator.sh |
| `yaml_get_schema_columns()` | 294-322 | Extract schema fields | Unique |

**Code Duplication**: ~170 lines duplicated between deploy.sh and ddl-generator.sh (lines 78-242 in ddl-generator.sh).

### 1.3 Configuration Sync Functions (Lines 335-1044)

| Function | Lines | Description | Complexity |
|----------|-------|-------------|------------|
| `sync_config()` | 335-359 | Sync to etcd | Medium |
| `init_streams()` | 361-373 | **DEPRECATED** - redirects to sync_config | Low |
| `sync_to_data_dictionary()` | 375-833 | Sync Bronze + Silver metadata to TimescaleDB | **HIGH** - 458 lines |
| `sync_domains_to_data_dictionary()` | 841-1044 | Sync domain objectives | **HIGH** - 203 lines |

### 1.4 Dimension Functions (Lines 1071-1335)

| Function | Lines | Description | Complexity |
|----------|-------|-------------|------------|
| `update_dimension_state()` | 1080-1093 | Track dimension sync state | Low |
| `get_dimension_state()` | 1097-1104 | Retrieve dimension state | Low |
| `import_dimension_sql()` | 1107-1163 | SQL COPY import fallback | Medium |
| `sync_dimension()` | 1166-1195 | Sync single dimension | Medium |
| `sync_dimensions()` | 1198-1259 | Sync all dimensions | Medium |
| `list_dimensions()` | 1262-1308 | List dimension status | Medium |
| `dimension_status()` | 1311-1335 | Show sync history | Low |

### 1.5 Core Lifecycle Functions (Lines 1337-1440)

| Function | Lines | Description | Complexity |
|----------|-------|-------------|------------|
| `build()` | 1337-1341 | Docker compose build | Low |
| `start()` | 1343-1358 | Start services + sync | Medium |
| `stop()` | 1360-1364 | Stop services | Low |
| `logs()` | 1366-1368 | Follow logs | Low |
| `status()` | 1390-1440 | Health checks + URLs | Medium |

### 1.6 Update Functions (Lines 1442-1542)

| Function | Lines | Description | Complexity |
|----------|-------|-------------|------------|
| `update()` | 1442-1506 | Git pull + rebuild + sync | Medium |
| `refresh()` | 1508-1542 | Git pull + sync (no rebuild) | Medium |

### 1.7 Declarative Deploy Functions (Lines 1544-2607)

| Function | Lines | Description | Complexity |
|----------|-------|-------------|------------|
| `validate_manifest()` | 1553-1624 | Validate manifest JSON | High |
| `validate_domain_configs()` | 1628-1694 | Validate domain configs with ndp-validate | Medium |
| `handle_stream()` | 1700-1737 | Process stream declaration | Medium |
| `handle_silver_table()` | 1741-1790 | Process Silver table declaration | Medium |
| `handle_migration()` | 1794-1813 | Apply SQL migration | Low |
| `handle_dimensions()` | 1817-1833 | Trigger dimension sync | Low |
| `handle_dictionary()` | 1837-1853 | Trigger data dictionary sync | Low |
| `handle_container_build()` | 1857-1888 | Build Docker images | Medium |
| `handle_tool()` | 1904-2001 | Build Rust CLI tools | **HIGH** - Docker fallback |
| `handle_gold_table()` | 2011-2082 | Generate + apply Gold DDL | High |
| `handle_domain()` | 2087-2211 | Sync domain config + aligned views | **HIGH** |
| `derive_correlation_role()` | 2219-2228 | Stream type to role mapping | Low |
| `derive_null_handling()` | 2232-2241 | Stream type to NULL handling | Low |
| `sync_stream_classification()` | 2245-2280 | Generate classification SQL | Medium |
| `sync_gold_table_metadata()` | 2284-2320 | Generate Gold metadata SQL | Medium |
| `handle_container_restart()` | 2324-2353 | Restart containers | Medium |
| `apply()` | 2357-2607 | **MAIN ORCHESTRATOR** - 11 phases | **VERY HIGH** |

---

## 2. Deployment Phases Breakdown

The `apply()` function (lines 2357-2607) orchestrates 11 deployment phases:

### Phase Breakdown Table

| Phase | Name | Lines | Handler | Dependencies |
|-------|------|-------|---------|--------------|
| 1 | Validation | 2366-2388 | `validate_manifest()` | jq, manifest file |
| 2 | Container Builds | 2401-2410 | `handle_container_build()` | Docker |
| 2.5 | Tool Builds | 2414-2423 | `handle_tool()` | Cargo or Docker |
| 3 | Migrations | 2426-2435 | `handle_migration()` | TimescaleDB, SQL files |
| 4 | Silver Tables | 2438-2447 | `handle_silver_table()` | DDL generator, TimescaleDB |
| 5 | Gold Tables | 2450-2464 | `handle_gold_table()` | ndp-gold-ddl tool |
| 6 | Domains | 2467-2481 | `handle_domain()` | ndp-gold-ddl, ndp-validate, etcd |
| 7 | Streams | 2484-2493 | `handle_stream()` | sync-streams-to-etcd.sh |
| 8 | Dimensions | 2496-2505 | `handle_dimensions()` | TimescaleDB |
| 9 | Dictionary | 2508-2517 | `handle_dictionary()` | TimescaleDB |
| 10 | Container Restarts | 2520-2529 | `handle_container_restart()` | Docker |
| 11 | Device State Update | 2532-2604 | Inline | /var/ndp filesystem |

### Phase Dependency Graph (Text)

```
                    [1. Validation]
                          |
                          v
               [2. Container Builds] ----+
                          |              |
                          v              |
               [2.5. Tool Builds] -------+
                          |              |
          +---------------+              |
          |               |              |
          v               v              |
[3. Migrations]    [4. Silver Tables]    |
          |               |              |
          +-------+-------+              |
                  |                      |
                  v                      |
          [5. Gold Tables] <-------------+
                  |
                  v
          [6. Domains]
                  |
                  v
          [7. Streams]
                  |
                  v
          [8. Dimensions]
                  |
                  v
          [9. Dictionary]
                  |
                  v
          [10. Container Restarts]
                  |
                  v
          [11. Device State Update]
```

---

## 3. External Dependencies

### 3.1 Required Tools

| Tool | Purpose | Required By | Fallback |
|------|---------|-------------|----------|
| `docker` | Container runtime | All | None - fatal |
| `docker compose` | Service orchestration | All | None - fatal |
| `jq` | JSON parsing | Manifest validation, handlers | Partial grep fallback |
| `git` | Version control | update, refresh, version | Degraded mode |

### 3.2 Optional Tools

| Tool | Purpose | Required By | Fallback |
|------|---------|-------------|----------|
| `yq` (Go or Python) | YAML parsing | yaml_* functions | Python fallback |
| `python3` | YAML parsing, complex parsing | yaml_* functions | grep/sed fallback |
| `cargo` | Rust compilation | handle_tool | Docker fallback |
| `ndp-gold-ddl` | Gold layer DDL | handle_gold_table | Skip with warning |
| `ndp-validate` | Config validation | validate_domain_configs | Skip with warning |
| `sha256sum` / `shasum` | Manifest hashing | Phase 11 | Degraded (no hash) |

### 3.3 Docker Services Depended Upon

| Service | Container | Health Check | Used By |
|---------|-----------|--------------|---------|
| etcd | etcd / integration-etcd | `etcdctl endpoint health` | sync_config, handle_stream, handle_domain |
| TimescaleDB | timescaledb | `pg_isready -U postgres -d ndp` | All Silver/Gold operations, migrations |
| Grafana | grafana | HTTP health endpoint | status, analytics |
| Air Quality App | air-quality-app | HTTP /health | status, container restarts |
| MCP Server | ndp-mcp-server | HTTP /health | status, container restarts |

### 3.4 External Scripts Called

| Script | Called From | Purpose |
|--------|-------------|---------|
| `scripts/sync-streams-to-etcd.sh` | sync_config, handle_stream | Stream config sync |
| `scripts/sync-config-to-etcd.sh` | sync_config | Legacy config sync |
| `deploy/pi/ddl-generator.sh` | sourced at line 109 | Silver DDL generation |
| `deploy/pi/configs/streams/list-streams.sh` | list-streams command | Stream listing |

---

## 4. Code Duplication Analysis

### 4.1 YAML Helper Functions (170+ lines)

**Location 1**: deploy.sh lines 120-290
**Location 2**: ddl-generator.sh lines 82-240

Functions duplicated:
- `yaml_get()` - 58 lines each
- `yaml_array_len()` - 50 lines each
- `yaml_array_get()` - 58 lines each

**Impact**: Bugs must be fixed in two places. Inconsistency risk.

### 4.2 Type Mapping Duplication

**Location 1**: deploy.sh lines 621-633 (inline in sync_to_data_dictionary)
**Location 2**: ddl-generator.sh lines 29-70 (`map_type()` function)

**Pattern**: Same PostgreSQL type mapping logic appears twice:
```bash
# deploy.sh (inline)
case "$col_type" in
    double_precision) pg_type="DOUBLE PRECISION" ;;
    smallint) pg_type="SMALLINT" ;;
    ...

# ddl-generator.sh (function)
map_type() {
    case "$config_type" in
        float|double_precision) echo "DOUBLE PRECISION" ;;
        real) echo "REAL" ;;
        ...
```

### 4.3 SQL Generation Patterns

Similar SQL generation patterns for UPSERT statements appear in:
- `sync_to_data_dictionary()` - lines 572-579, 647-656, 665-671, 709-714
- `sync_domains_to_data_dictionary()` - lines 908-916, 979-981, 1014-1016
- `sync_stream_classification()` - lines 2269-2279
- `sync_gold_table_metadata()` - lines 2307-2319

---

## 5. Code Smell Identification

### 5.1 Complexity Smells

| Smell | Location | Lines | Issue |
|-------|----------|-------|-------|
| God Function | `sync_to_data_dictionary()` | 375-833 (458 lines) | Too many responsibilities |
| God Function | `apply()` | 2357-2607 (250 lines) | Orchestrates 11 phases inline |
| SQL String Building | Throughout | 200+ occurrences | Injection risk, hard to maintain |
| Bash Arrays | Lines 488-491 | `declare -A` | Limited bash 4+ support |

### 5.2 Maintainability Smells

| Smell | Evidence | Impact |
|-------|----------|--------|
| Magic Numbers | `timeout=${2:-60}`, `sleep 2`, `sleep 10` | Scattered timing values |
| Hardcoded Paths | `/var/ndp`, `/opt/ndp/bin/` | Non-configurable locations |
| Comment-Heavy Code | Lines 471-498 | Complex logic requires extensive comments |
| Inline SQL Generation | Lines 390-816 | Multi-hundred line SQL generation |

### 5.3 Error Handling Smells

| Pattern | Frequency | Issue |
|---------|-----------|-------|
| `2>/dev/null` suppression | 50+ times | Hides real errors |
| `|| true` ignoring failures | 20+ times | Silent failures |
| `|| echo "0"` default values | 15+ times | Masks jq failures |
| No validation after SQL exec | 10+ times | Assumes success |

### 5.4 Security Considerations

| Issue | Location | Severity |
|-------|----------|----------|
| SQL string concatenation | sync_* functions | Medium - injection risk |
| Password in db_url | Line 2049 | Medium - process visible |
| No input sanitization | handle_* functions | Low - manifest is trusted |
| `set -e` with `|| true` | Throughout | Low - inconsistent behavior |

---

## 6. Complexity Metrics

### 6.1 File Size Distribution

| File | Lines | Functions | Avg Lines/Function |
|------|-------|-----------|-------------------|
| deploy.sh | 2,868 | 43 | 66.7 |
| ddl-generator.sh | 850 | 14 | 60.7 |
| list-streams.sh | 135 | 2 | 67.5 |
| init-streams.sh | 81 | 2 | 40.5 |
| add-stream.sh | 117 | 3 | 39.0 |
| **Total** | **4,051** | **64** | **63.3** |

### 6.2 Cyclomatic Complexity Indicators

**High complexity functions** (estimated by case/if nesting):

| Function | Est. Cyclomatic | Contributors |
|----------|-----------------|--------------|
| `sync_to_data_dictionary()` | 35+ | 8 nested loops, 15+ conditions |
| `apply()` | 25+ | 11 phase blocks, multiple conditions |
| `sync_domains_to_data_dictionary()` | 20+ | 5 loops, 10+ conditions |
| `handle_domain()` | 15+ | Multiple tool checks, file checks |
| `handle_gold_table()` | 12+ | Tool discovery, error paths |

### 6.3 Lines of Code by Category

| Category | Lines | Percentage |
|----------|-------|------------|
| YAML Helpers | 200 | 7% |
| SQL Generation | 600 | 21% |
| Config Sync | 500 | 17% |
| Declarative Deploy | 700 | 24% |
| Docker Operations | 300 | 10% |
| Silver ETL | 200 | 7% |
| Utilities/Logging | 100 | 4% |
| Help/Documentation | 100 | 4% |
| Main Case Statement | 168 | 6% |

---

## 7. Manifest System Analysis

### 7.1 Supported Declaration Types

| Type | Handler | Status | Notes |
|------|---------|--------|-------|
| `stream` | `handle_stream()` | Active | Syncs via external script |
| `silver-table` | `handle_silver_table()` | Active | Uses DDL generator |
| `gold-tables` | `handle_gold_table()` | Active | Requires ndp-gold-ddl |
| `domain` | `handle_domain()` | Active | Requires ndp-gold-ddl |
| `migration` | `handle_migration()` | Active | SQL file application |
| `dimensions` | `handle_dimensions()` | Active | Triggers sync_dimensions |
| `dictionary` | `handle_dictionary()` | Active | Triggers sync_to_data_dictionary |
| `container` (build) | `handle_container_build()` | Active | Docker build |
| `container` (restart) | `handle_container_restart()` | Active | Docker restart |
| `tool` | `handle_tool()` | Active | Cargo/Docker build |

### 7.2 Manifest Format Evolution

**Version 1.0 (.changes array):**
```json
{
  "version": "1.0",
  "release_version": "1.1.2",
  "changes": [
    {"type": "stream", "id": "...", "action": "update"}
  ]
}
```

**Planned (.declarations object):**
```json
{
  "version": "1.0",
  "declarations": {
    "streams": [...],
    "gold-tables": [...],
    "domains": [...]
  }
}
```

The code supports both formats for backward compatibility (lines 2391-2398, 2451-2454, 2468-2471).

---

## 8. Architectural Observations

### 8.1 Evolution Pattern

The script shows clear evolutionary layers:
1. **Layer 1 (Original)**: Basic Docker lifecycle (deploy, start, stop, logs)
2. **Layer 2 (dp-013/dp-018)**: Stream config management, etcd sync
3. **Layer 3 (dp-020)**: Declarative deployment with manifest
4. **Layer 4 (fe-001)**: Gold layer handlers, tool builds
5. **Layer 5 (fe-002/v11)**: Domain objectives, classification

### 8.2 Emerging Patterns

**Tool Delegation**: Complex logic being extracted to Rust tools:
- `ndp-gold-ddl` - Gold layer DDL generation
- `ndp-validate` - Configuration validation
- External script `sync-streams-to-etcd.sh`

**Database as State Store**: Heavy reliance on TimescaleDB for:
- Data dictionary metadata
- Stream classification
- Domain objectives
- Sync status tracking

### 8.3 Constraints for Refactoring

1. **No native language on Pi**: Bash is the only guaranteed language
2. **Docker always available**: Can use containerized tools
3. **Backward compatibility**: Must support existing manifests
4. **Idempotency required**: All operations must be re-runnable

---

## 9. Recommendations Summary

### 9.1 High Priority

1. **Extract SQL generation**: Move complex SQL building to templates or Rust tool
2. **Consolidate YAML helpers**: Single source of truth for YAML parsing
3. **Add error handling**: Replace `|| true` with explicit error handling

### 9.2 Medium Priority

1. **Split deploy.sh**: Extract modules (sync, silver, gold, etc.)
2. **Configuration externalization**: Move hardcoded paths to config
3. **Logging improvements**: Structured logging for automation

### 9.3 Future Considerations

1. **Rust CLI for deployment**: `ndp-deploy` to replace bash orchestration
2. **State machine model**: Formal phase transitions with rollback
3. **Manifest schema validation**: JSON Schema for manifest files

---

## Appendix A: Command Reference

| Command | Function(s) Called | Lines |
|---------|-------------------|-------|
| `deploy` | check_prereqs, build, start | 2690-2694 |
| `start` | check_prereqs, start | 2695-2698 |
| `stop` | stop | 2699-2701 |
| `logs` | logs | 2702-2704 |
| `status` | status | 2705-2707 |
| `update` | check_prereqs, update | 2708-2712 |
| `refresh` | refresh | 2713-2715 |
| `build` | check_prereqs, build | 2716-2719 |
| `sync` | sync_config | 2720-2722 |
| `init-streams` | init_streams | 2723-2725 |
| `list-streams` | External script | 2726-2732 |
| `sync-dictionary` | sync_to_data_dictionary | 2733-2735 |
| `sync-domains` | sync_domains_to_data_dictionary | 2736-2738 |
| `list-domains` | list_domains | 2739-2741 |
| `apply` | apply | 2742-2760 |
| `sync-dimensions` | sync_dimensions | 2761-2763 |
| `list-dimensions` | list_dimensions | 2764-2766 |
| `dimension-status` | dimension_status | 2767-2769 |
| `analytics` | Docker operations | 2770-2778 |
| `rollback` | Docker operations | 2779-2785 |
| `silver-etl` | Docker profile run | 2786-2796 |
| `silver-migrate` | Docker profile run | 2797-2807 |
| `silver-daemon` | Docker profile up | 2808-2819 |
| `silver-daemon-stop` | Docker profile stop/rm | 2820-2825 |
| `silver-daemon-logs` | docker logs | 2826-2829 |
| `silver-daemon-status` | docker ps/exec/logs | 2830-2842 |
| `version` | State file reading | 2843-2861 |

---

## Appendix B: File Relationships

```
deploy/pi/
├── deploy.sh (2868 lines) ─────────────────────┐
│   ├── sources ddl-generator.sh                │
│   ├── calls configs/streams/list-streams.sh   │
│   └── calls scripts/sync-*.sh                 │
│                                               │
├── ddl-generator.sh (850 lines) ──────────────┤
│   └── Standalone or sourced                   │
│                                               │
├── docker-compose.yml ────────────────────────┤
│                                               │
├── configs/                                    │
│   └── streams/                                │
│       ├── init-streams.sh (81 lines) DEPRECATED
│       ├── list-streams.sh (135 lines)         │
│       └── add-stream.sh (117 lines) DEPRECATED│
│                                               │
└── init-scripts/                               │
    └── *.sql (migrations) ─────────────────────┘

.deploy/releases/
├── TEMPLATE.manifest.json
├── v1.0.0.manifest.json
├── v1.1.0.manifest.json
├── ... (10 manifests total)
└── v1.1.8.manifest.json (latest)
```

---

*Analysis complete. This document provides a foundation for architectural decisions regarding deployment system evolution.*
