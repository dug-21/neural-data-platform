# ops-002: Eliminate Hardcoded References from Gold Layer Generators

## Current Phase
release

## Progress
- [x] SCOPE.md created
- [x] SPARC Specification complete
- [x] SPARC Pseudocode complete
- [x] SPARC Architecture complete
- [x] SPARC Refinement complete (tests-first approach defined in COMPLETION.md)
- [x] SPARC Completion criteria defined
- [x] Phase 1: Constants module (NDP_ENTITY_COLUMN, GOLD_SCHEMA) — DONE
- [x] Phase 2: StreamConfig.stream_type + TransitionConfig fixes — DONE
- [x] Phase 3: EventsGenerator config-driven refactor — DONE
- [x] Phase 4: Test suite (hardcoding detection, London TDD) — DONE
- [x] Phase 5: Integration verification — DONE
- [x] Bugfix: Duplicate CTE names in detection procedure — DONE
- [x] All tests passing (616 total: 399 ndp-gold-ddl + 217 ndp-validate)
- [x] Release artifacts created (v1.1.11 manifest, CHANGELOG)
- [x] Deployed to production
- [x] BUG-002: Domain objectives sync migrated to Rust CLI — DONE
- [x] Release v1.1.12 artifacts created (manifest, CHANGELOG, tag)
- [x] E2E verified in integration environment

## Active Work
Feature complete. v1.1.12 released.

## Scope Summary
Eliminate 50+ hardcoded domain-specific values from Gold DDL generators (`events.rs`, `state_transitions.rs`, `aligned_view.rs`). Replace with config-driven reads from `DomainConfig`, `StreamConfig`, and `TransitionConfig`.

## Implementation Phases
| Phase | Description | Status | Dependencies |
|-------|-------------|--------|-------------|
| 1 | Test Infrastructure (London TDD) | DONE | None |
| 2 | Shared Constants (`NDP_ENTITY_COLUMN`, `GOLD_SCHEMA`) | DONE | Phase 1 |
| 3 | EventsGenerator Refactoring (P0) | DONE | Phase 2 |
| 4 | StateTransitionGenerator Refactoring (P0) | DONE | Phase 2 |
| 5 | AlignedView Fix (P0) | DONE | Phase 2 |
| 6 | Integration Verification | DONE | Phases 3, 4, 5 |
| 7 | Bugfix: Duplicate CTE names | DONE | Phase 6 |
| 8 | Release (v1.1.11) | READY | Phase 7 |

## Test Targets
| Metric | Target | Current |
|--------|--------|---------|
| Existing tests passing | 556+ | 616 |
| New tests (ndp-gold-ddl: 399 - 339 baseline) | >= 15 | 60 |
| Hardcoded domain literals in generators | 0 | See notes below |
| Fictional domain test (energy-monitoring) | PASS | N/A |

## Remaining Hardcoded Values (Post-Verification)
- **events.rs**: Air-quality references remain only in test code (`MockConfigLoader::air_quality_loader()` and test assertions). Production code path is config-driven.
- **state_transitions.rs**: `"on"/"off"` and `"door_%"/"window_%"` appear only in test fixtures and assertions (lines 524, 700, 767, 771, 794-795). The `TransitionConfig` struct is config-driven (fields at lines 21, 58).
- **aligned_view.rs**: Heuristic fallback at line 120-137 (`determine_stream_type`) uses string matching on stream_id ("forecast", "state", "event", "dimension", "ref") as backward-compatible fallback when `stream_config.stream_type` is not set. Config-driven path takes priority.

## Bugs
| ID | Status | Summary |
|----|--------|---------|
| BUG-001 | FIXED | Duplicate CTE names in detection procedure when objectives share metric (humidity_pct, temperature_c) |
| BUG-002 | FIXED | Domain objectives sync migrated from dead Bash to `ndp domain sync` Rust CLI (v1.1.12). 18 London TDD tests, E2E verified. |

## Release Target
- **Version:** v1.1.11 (PATCH -- refactoring, no new features)
- **Manifest:** `.deploy/releases/v1.1.11.manifest.json`
- **Predecessor:** v1.1.10 (EventsGenerator wiring)

## Branch
`feature/ops-002`

## Key Files (17 modified)
- `tools/ndp-gold-ddl/src/generators/events.rs` -- config-driven refactor (+1509/-342 lines)
- `tools/ndp-gold-ddl/src/generators/state_transitions.rs` -- TransitionConfig-driven (+192 lines)
- `tools/ndp-gold-ddl/src/generators/aligned_view.rs` -- config fallback with heuristic (+36 lines)
- `tools/ndp-gold-ddl/src/config/loader.rs` -- ConfigLoader updates
- `tools/ndp-gold-ddl/src/config/types.rs` -- new config types
- `tools/ndp-gold-ddl/src/main.rs` -- EventsGenerator wired with FileSystemConfigLoader

## Clippy Status
- ndp-gold-ddl: 3 warnings (vec_init_then_push, 2x trim_split_whitespace) -- pre-existing, not from ops-002
- ndp-validate: clean
- Workspace clippy cannot run (missing protoc for etcd-client build)

## Last Updated
2026-02-06 by ndp-rust-dev (v1.1.12 BUG-002 fix released)
