# AIR-008: Home Events

## Current Phase
specification

## Progress
- [x] SCOPE.md created
- [ ] SPARC Specification complete
- [ ] SPARC Pseudocode complete
- [ ] SPARC Architecture complete
- [ ] SPARC Refinement complete
- [ ] SPARC Completion complete
- [ ] All tests passing
- [ ] Documentation updated
- [ ] Deployed to production

## Active Work
Researching data architecture strategies for event-based vs state-change data models. Evaluating Home Assistant data approach and broader time-series platform patterns.

## Key Decisions Pending

| Decision | Options | Status |
|----------|---------|--------|
| Data Model | Event-based vs State-change | Research in progress |
| Home Assistant Integration | Direct API vs MQTT vs Polling | To be evaluated |
| Storage Strategy | Separate event store vs Unified time-series | To be determined |
| Future Extensibility | Log streams, generic events | Architecture consideration |

## Dependencies
- Home Assistant instance with window sensor data
- Existing air quality monitoring streams (AIR-001 through AIR-007)
- TimescaleDB Silver layer (dp-001, future)

## Research Artifacts
Research output will be stored in: `product/research/dp-analysis/`

## Bugs
| ID | Status | Summary |
|----|--------|---------|

## Branch
`feature/air-008`

## Last Updated
2025-12-29 10:00 by ndp-scrum-master
