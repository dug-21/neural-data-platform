# AIR-014 Status

## Current Phase: Specification

## Summary

Self-healing Silver ETL with automatic catch-up from Bronze after TimescaleDB failures.

## Progress

| Phase | Status | Notes |
|-------|--------|-------|
| Scope | Complete | Problem identified via code investigation |
| Specification | Not Started | |
| Pseudocode | Not Started | |
| Architecture | Not Started | |
| Refinement | Not Started | |
| Completion | Not Started | |

## Key Decisions

- **Trigger**: Decided to investigate based on observed gap behavior during DB outage
- **Approach**: Circuit breaker + Bronze catch-up (no external queue dependencies)

## Open Questions

1. Should circuit breaker be a shared component or SilverSubscriber-specific?
2. What's the appropriate default catch-up window given Bronze retention policies?
3. Should we pause live event processing during catch-up or run in parallel?

## Investigation Summary (2026-02-04)

**Finding**: SilverSubscriber IS running in production (embedded in air-quality-app), not as separate daemon.

**Current failure behavior**:
- `silver.rs:560-563`: Errors logged but events dropped
- `NoBronzeReader` stub used - catch-up disabled
- No retry, no buffer, no dead letter queue
- Result: Data gap during any TimescaleDB unavailability

**Existing code to leverage**:
- `BronzeReader` trait already defined
- `CatchUpConfig` struct exists (just disabled)
- `load_watermark()` / `save_watermark()` methods exist
- `ParquetStore` can read Bronze data
