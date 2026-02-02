# dp-020: Declarative Deploy

## Current Phase
refinement (integration testing)

## Progress
- [x] SCOPE.md created
- [x] SPARC Specification
- [x] SPARC Pseudocode
- [x] SPARC Architecture
- [x] SPARC Refinement
- [ ] SPARC Completion
- [x] All tests passing
- [ ] Documentation updated

---

## Task Progress

### Manifest and Orchestration

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 3.1 | Define manifest schema | Complete | schemas/manifest.schema.json created |
| 3.2 | Create manifest parser | Complete | validate_manifest() in deploy.sh |
| 3.9 | Create deploy.sh apply | Complete | 9-phase orchestration implemented |
| 3.10 | Add device state tracking | Complete | Phase 9: version + timestamp |

### Action: Stream Sync

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 3.3 | Implement stream sync | Complete | handle_stream() syncs all streams |

### Action: Silver Table DDL (dp-015)

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 3.4 | Implement silver-table action | Complete | handle_silver_table() |
| 3.4a | DDL generator: CREATE TABLE | Complete | generate_create_table_ddl() |
| 3.4b | DDL generator: Indexes | Complete | generate_indexes_ddl() |
| 3.4c | DDL generator: Hypertable | Complete | generate_hypertable_ddl() |
| 3.4d | DDL generator: Policies | Complete | generate_policies_ddl() |
| 3.4e | DDL generator: Permissions | Complete | generate_permissions_ddl() - idempotent |
| 3.4f | Idempotent execution | Complete | IF NOT EXISTS, role checks |

### Action: Other Declarations

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 3.5 | Implement migration action | Complete | handle_migration() |
| 3.6 | Implement dimensions action | Complete | handle_dimensions() |
| 3.7 | Implement dictionary action | Complete | handle_dictionary() |
| 3.8 | Implement reload logic | Partial | reload field parsed but not yet applied |

---

## Integration Test Results

All tests passing (2026-02-02):

| Test | Result | Notes |
|------|--------|-------|
| T1: DDL generation | PASS | CREATE TABLE with correct types |
| T2: Type mapping | PASS | float→DOUBLE PRECISION |
| T3: Indexes | PASS | GIN, composite indexes created |
| T4: Hypertable | PASS | TimescaleDB conversion successful |
| T5: Policies | PASS | Compression/retention added |
| T6: Permissions | PASS | Idempotent role checks |
| T7: Stream sync | PASS | 8/8 streams synced |
| T8: Dictionary sync | PASS | 7 streams, 62 attrs |
| T9: Idempotency | PASS | IF NOT EXISTS working |

---

## Files Modified/Created

| File | Type | Purpose |
|------|------|---------|
| schemas/manifest.schema.json | Created | JSON Schema for manifest validation |
| deploy/pi/ddl-generator.sh | Created | DDL generation functions |
| deploy/pi/deploy.sh | Modified | Added apply command, 9-phase orchestration |
| .deploy/manifest.json | Created | Template manifest |
| config/base/streams/_test-dp020/config.json | Created | Test stream config |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-018 | Complete | JSON configs, ConfigLoader |
| dp-019 | Complete | Validation, type mapping |
| dp-017 | Complete | Integration environment ready |

---

## Absorbs

- **dp-015**: Config-Driven Silver Table Creation (now absorbed into dp-020)

---

## Branch
main (trunk-based development)

## Last Updated
2026-02-02
