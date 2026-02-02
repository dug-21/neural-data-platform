# dp-022: MCP Write Tools

## Parent Initiative

This feature implements **Phase 6** of [dp-016: Configuration Architecture Review](../dp-016/IMPLEMENTATION-ROADMAP.md).

**Moved from**: dp-021 (2026-02-02)

---

## Problem Statement

Agents can read configuration via MCP but cannot create, update, or delete stream configurations programmatically. Full MCP administration capability enables AI-driven configuration management.

---

## Goals

1. **CRUD operations** - Create, read, update, delete stream configs via MCP
2. **Validation integration** - All writes validated before persistence
3. **Deploy integration** - Changes trigger declarative deploy pipeline

---

## Scope

### Phase 6: MCP Write Tools

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 6.1 | create_stream MCP tool | Create new stream config via MCP | Writes JSON, triggers deploy |
| 6.2 | update_stream MCP tool | Modify existing stream config | Validates before save |
| 6.3 | delete_stream MCP tool | Remove stream config | Cleans up etcd, optional table drop |
| 6.4 | validate_stream MCP tool | Dry-run validation | Returns validation errors |
| 6.5 | create_silver_table MCP tool | Generate and apply DDL | Creates table from config |
| 6.6 | reload_stream MCP tool | Trigger hot-reload | For source-level changes |

---

## MCP Tool Flow

```
MCP Tool (create_stream)
    │
    ├── Validate JSON against schema (dp-019)
    ├── Write to config/base/streams/{id}/config.json
    ├── Update .deploy/manifest.json
    │
    ▼
Trigger deploy.sh apply (dp-020)
    │
    ▼
Git commit/push (backup)
```

---

## Deliverables

| Deliverable | Location | Description |
|-------------|----------|-------------|
| MCP write tools | `apps/ndp-mcp-server/src/tools/` | 6 new MCP tools |
| Tool schemas | `apps/ndp-mcp-server/schemas/` | JSON schemas for tool inputs |
| Integration tests | `tests/integration/mcp/` | MCP tool verification |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-019 | **Complete** | Validation pipeline |
| dp-020 | **Complete** | Declarative deploy |
| dp-021 | **REQUIRED** | Hot-reload for reload_stream tool |
| **Access Controls** | **REQUIRED** | MCP write functions blocked until access control design |

### Access Control Prerequisite

MCP write tools are blocked until access controls are designed. This includes:
- Authentication/authorization for MCP clients
- Role-based access (read-only vs admin)
- Audit logging for configuration changes

This may become a separate feature (dp-023 or later).

---

## Future Architecture

The long-term architecture consolidates NDP tools as **Rust crates** callable from both CLI and MCP:

```
┌─────────────────────────────────────────────────┐
│              Rust Library Crates                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ validate │ │ migrate  │ │  sync    │  ...   │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘        │
└───────┼────────────┼────────────┼───────────────┘
        │            │            │
   ┌────┴────┐  ┌────┴────┐  ┌────┴────┐
   │   CLI   │  │   CLI   │  │   CLI   │
   │ wrapper │  │ wrapper │  │ wrapper │
   └─────────┘  └─────────┘  └─────────┘
        │            │            │
   ┌────┴────┐  ┌────┴────┐  ┌────┴────┐
   │   MCP   │  │   MCP   │  │   MCP   │  ← with access controls
   │  tool   │  │  tool   │  │  tool   │
   └─────────┘  └─────────┘  └─────────┘
```

This enables code reuse and consistent behavior across interfaces.

---

## Success Criteria

1. **create_stream** creates valid config and triggers deploy
2. **update_stream** validates before saving
3. **delete_stream** cleans up etcd and optionally drops table
4. **All tools return** structured success/error responses

---

## Related Features

| Feature | Relationship |
|---------|--------------|
| dp-016 | Parent: Configuration Architecture Review |
| dp-019 | Prerequisite: Validation Pipeline |
| dp-020 | Prerequisite: Declarative Deploy |
| dp-021 | Prerequisite: Hot-Reload, Release Methodology |

---

*Scope created: 2026-02-02*
*Moved from: dp-021 Phase 6*
*Parent: dp-016 Configuration Architecture Review*
