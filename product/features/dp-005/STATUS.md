# dp-005: Bronze MCP Server - Status

## Current Phase: Specification

| Phase | Status | Notes |
|-------|--------|-------|
| Specification | 🟢 Complete | Scope finalized, pending approval |
| Pseudocode | ⬜ Not Started | |
| Architecture | ⬜ Not Started | ADRs pending |
| Refinement | ⬜ Not Started | |
| Completion | ⬜ Not Started | |

## Summary

Rust-based MCP server exposing Bronze layer data exploration and config validation tools. Validates the full config pipeline: source YAML → etcd → MCP → agent.

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

## All Questions Resolved

1. ✅ etcd schema: Flattened YAML at `/streams/{stream_id}/*`
2. ✅ File organization: Hive-style `/data/raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet` (one file per partition)
3. ✅ Schema sources: Multiple - Bronze (Parquet), Mappings (parser config), Silver (entity_schemas)
4. ✅ Validation: Field name comparison, pretty JSON output
5. ✅ Sample data: Most recent N rows
6. ✅ Transport: HTTP POST
7. ✅ etcd unavailability: Fail fast
8. ✅ Stream discovery: Hybrid (etcd + filesystem)

## Artifacts

| Artifact | Location | Status |
|----------|----------|--------|
| Scope | `SCOPE.md` | Complete |
| MCP Patterns | `specification/mcp-design-patterns.md` | Complete |
| Research | `/research/agenticdataplatform/` | Complete |
| MCP Reference | [gist](https://gist.github.com/ruvnet/ea1ec6678b1552c3ff3ae92dc1001d23) | Reviewed |

## Next Steps

1. Review and approve SCOPE.md
2. Begin Architecture phase (ADRs)
3. Pseudocode for tool implementations

---

*Last updated: 2026-01-03*
