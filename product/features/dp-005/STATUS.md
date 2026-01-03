# dp-005: Bronze MCP Server - Status

## Current Phase: SPARC Documentation Complete - Ready for Implementation

| Phase | Status | Notes |
|-------|--------|-------|
| Specification | 🟢 Complete | 10 artifacts: requirements, interfaces, data contracts, dependencies, tests, **deployment** |
| Pseudocode | 🟢 Complete | Algorithm design for 4 tools, server lifecycle, error handling |
| Architecture | 🟢 Complete | **6 ADRs** + consolidated ARCHITECTURE.md (includes deployment) |
| Refinement | 🟢 Complete | Test strategy, success criteria, implementation phases, acceptance checklist (**with deployment testing**) |
| Completion | ⬜ Not Started | TDD implementation pending |

## Summary

Rust-based MCP server exposing Bronze layer data exploration and config validation tools. Validates the full config pipeline: source YAML → etcd → MCP → agent.

## Deployment Scope Expansion - COMPLETED

**Date**: 2026-01-03
**Status**: All deployment documentation added to SPARC artifacts

The original SPARC specification did not include deployment details. A specialized swarm was deployed to update all artifacts with:

| Update | Artifact | Description |
|--------|----------|-------------|
| ADR-006 | `architecture/ADR-006-deployment-strategy.md` | Deployment decisions: port, memory, dependencies |
| Deployment Reqs | `specification/deployment-requirements.md` | Container, Docker Compose, deploy.sh requirements |
| Architecture | `architecture/ARCHITECTURE.md` section 6 | Complete docker-compose service definition |
| Implementation | `refinement/implementation-phases.md` | Phase 4 expanded with deployment deliverables |
| Test Strategy | `refinement/TEST-STRATEGY.md` | Deployment tests (DT-001 to DT-023) |
| Acceptance | `refinement/acceptance-checklist.md` | Expanded Pi Deployment checklist |
| Success | `refinement/success-criteria.md` | Deployment success criteria |
| Specification | `specification/SPECIFICATION.md` | Deployment requirements summary |

### Deployment Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Memory limit | 64MB | <50MB target + buffer for Parquet read spikes |
| Port | 9100 | Non-conflicting, metrics-adjacent range |
| Volume | air-quality-data:/data/raw:ro | Read-only access to Bronze layer |
| Dependencies | etcd (service_healthy) | Config source, fail-fast |
| Health check | GET /health, 30s interval | Matches existing service patterns |

---

## SPARC Documentation Sprint - COMPLETED

**Started**: 2026-01-03
**Completed**: 2026-01-03
**Status**: All SPARC documentation phases complete (including deployment)

### Agents Deployed

| Agent | Role | Assignment | Status |
|-------|------|------------|--------|
| ndp-scrum-master | Coordinator | STATUS.md, reports, coordination | ✅ Complete |
| ndp-architect | Architecture | ADRs, trait design, MCP protocol decisions | ✅ Complete |
| specification-agent | Specification | SPECIFICATION.md formal requirements | ✅ Complete |
| pseudocode-agent | Pseudocode | Algorithm design | ✅ Complete |
| ndp-tester | Testing | Test strategy, acceptance criteria | ✅ Complete |
| refinement-agent | Refinement | TDD plan, implementation readiness | ✅ Complete |

### SPARC Artifacts - All Complete

| Artifact | Location | Status |
|----------|----------|--------|
| **SPECIFICATION.md** | `specification/SPECIFICATION.md` | 🟢 Complete |
| requirements.md | `specification/requirements.md` | 🟢 Complete |
| interfaces.md | `specification/interfaces.md` | 🟢 Complete |
| data-contracts.md | `specification/data-contracts.md` | 🟢 Complete |
| dependencies.md | `specification/dependencies.md` | 🟢 Complete |
| test-plan.md | `specification/test-plan.md` | 🟢 Complete |
| test-cases.md | `specification/test-cases.md` | 🟢 Complete |
| test-fixtures.md | `specification/test-fixtures.md` | 🟢 Complete |
| mcp-design-patterns.md | `specification/mcp-design-patterns.md` | 🟢 Complete |
| deployment-requirements.md | `specification/deployment-requirements.md` | 🟢 Complete |
| **ARCHITECTURE.md** | `architecture/ARCHITECTURE.md` | 🟢 Complete |
| ADR-001 MCP Transport | `architecture/ADR-001-mcp-transport.md` | 🟢 Complete |
| ADR-002 Storage Abstraction | `architecture/ADR-002-storage-abstraction.md` | 🟢 Complete |
| ADR-003 Config Source | `architecture/ADR-003-config-source.md` | 🟢 Complete |
| ADR-004 Schema Discovery | `architecture/ADR-004-schema-discovery.md` | 🟢 Complete |
| ADR-005 Response Format | `architecture/ADR-005-response-format.md` | 🟢 Complete |
| ADR-006 Deployment Strategy | `architecture/ADR-006-deployment-strategy.md` | 🟢 Complete |
| tool-algorithms.md | `pseudocode/tool-algorithms.md` | 🟢 Complete |
| server-lifecycle.md | `pseudocode/server-lifecycle.md` | 🟢 Complete |
| error-handling.md | `pseudocode/error-handling.md` | 🟢 Complete |
| **TEST-STRATEGY.md** | `refinement/TEST-STRATEGY.md` | 🟢 Complete |
| success-criteria.md | `refinement/success-criteria.md` | 🟢 Complete |
| implementation-phases.md | `refinement/implementation-phases.md` | 🟢 Complete |
| acceptance-checklist.md | `refinement/acceptance-checklist.md` | 🟢 Complete |

### Patterns Saved to AgentDB

| Pattern | Category | Description |
|---------|----------|-------------|
| `arch-mcp-http-transport` | architecture | HTTP POST with axum, single /mcp endpoint |
| `arch-bronze-storage-trait` | architecture | BronzeStorage trait for local/cloud portability |
| `arch-mcp-etcd-config` | architecture | ConfigStore reading from etcd |
| `arch-parquet-introspection` | architecture | Dynamic schema discovery from Parquet |
| `arch-mcp-response-format` | architecture | Consistent JSON with success flag |
| `arch-mcp-deployment-strategy` | deployment | Docker Compose integration, 64MB limit, port 9100 |

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Rust | CPU/memory efficiency on edge |
| Transport | HTTP POST | Cloud-portable, standard |
| Config source | etcd | Validates full config pipeline |
| Schema sources | Multiple, domain-specific | Bronze=Parquet, Mappings=parser config, Silver=entity_schemas |
| Storage abstraction | Trait-based | Enable S3/GCS later |
| Validation scope | Field names only | Bronze stores raw JSON; type parsing deferred |
| Sample behavior | Most recent N rows | Simple, adequate for MVP |
| etcd unavailability | Fail fast | No stale config for validation |
| Stream discovery | Hybrid | etcd for metadata, filesystem for storage stats |
| File organization | Hive-style | `year=YYYY/month=MM/day=DD` partitioning |
| **Deployment** | Docker Compose | Integrate with existing deploy.sh and docker-compose.yml |
| **Memory limit** | 64MB | <50MB target + buffer for Parquet read spikes |
| **Port** | 9100 | Non-conflicting, metrics-adjacent range |
| **Volume** | air-quality-data:ro | Reuse existing Bronze data volume |

## All Questions Resolved

1. etcd schema: Flattened YAML at `/streams/{stream_id}/*`
2. File organization: Hive-style `/data/raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet` (one file per partition)
3. Schema sources: Multiple - Bronze (Parquet), Mappings (parser config), Silver (entity_schemas)
4. Validation: Field name comparison, pretty JSON output
5. Sample data: Most recent N rows
6. Transport: HTTP POST
7. etcd unavailability: Fail fast
8. Stream discovery: Hybrid (etcd + filesystem)
9. **Deployment**: Docker Compose integration with existing stack
10. **Memory limit**: 64MB (includes buffer for Parquet reads)
11. **Port**: 9100 (non-conflicting, accessible from Mac)
12. **Volume**: Read-only access to air-quality-data volume
13. **Health check**: GET /health with 30s interval

## Completed Artifacts

| Artifact | Location | Status |
|----------|----------|--------|
| Scope | `SCOPE.md` | Complete |
| MCP Patterns | `specification/mcp-design-patterns.md` | Complete |
| Research | `/research/agenticdataplatform/` | Complete |
| MCP Reference | [gist](https://gist.github.com/ruvnet/ea1ec6678b1552c3ff3ae92dc1001d23) | Reviewed |
| SPARC Kickoff | `reports/sparc-kickoff.md` | Complete |

## Bugs

| ID | Status | Summary |
|----|--------|---------|
| - | - | No bugs reported yet |

## Branch

`main` (Trunk-Based Development per `ndp-github-workflow`)

## Next Steps

1. ✅ ~~Complete SPARC documentation artifacts~~ - DONE
2. Review ADRs with team (optional)
3. Begin TDD implementation (Completion phase) when ready
   - Follow `refinement/implementation-phases.md` for phase order
   - Use `refinement/TEST-STRATEGY.md` for test approach
   - Reference `refinement/success-criteria.md` for acceptance

---

*Last updated: 2026-01-03 by deployment scope expansion swarm*
