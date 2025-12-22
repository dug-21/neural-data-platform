# AIR-006: Config-Driven Stream Implementation + NWS Integration

## Current Phase
**Phase**: Refinement (Implementation)
**Status**: PARTIALLY IMPLEMENTED - Build passes but tests failing
**Last Updated**: 2025-12-22 18:30 UTC
**Sprint**: TBD
**Assigned Agents**: ndp-rust-dev (lead), ndp-tester, ndp-scrum-master

## Scope Evolution

**Original Scope**: NWS Weather Data Integration (2 new streams)

**Expanded Scope**: This feature evolved from BUG-002 into a comprehensive Bronze layer enhancement:

1. **Fix BUG-002**: Complete config-driven parsing (was partially implemented)
2. **Add Array Iteration**: New `array_iterator` parser_type for NWS forecast periods
3. **Unify Parser System**: Integrate Parser trait into all sources, remove ResponseParser duplication
4. **Add NWS Streams**:
   - nws-observations-ksgj (current weather)
   - nws-forecast-jax (hourly forecast with array iteration)
5. **Clean Legacy Code**: Remove hardcoded parsers after migration

**Scope Boundary**: Bronze layer only - Silver layer ETL in separate feature (dp-XXX)

## SPARC Progress

### S - Specification ✅ COMPLETE
- [x] SCOPE.md created and revised for expanded scope
- [x] STATUS.md tracking (this file)
- [x] BUG-002 requirements analysis
- [x] NWS API research completed
  - [x] Station KSGJ validated
  - [x] Grid point JAX/79,49 confirmed
  - [x] Observations endpoint tested
  - [x] Hourly forecast endpoint tested
- [x] Specification documents created:
  - [x] `specification/SPECIFICATION.md` - Complete requirements
  - [x] `specification/BUG-002-SPECIFICATION.md` - Config-driven parsing requirements
  - [x] `specification/STREAM-CONFIGS.md` - All 5 stream configurations

### P - Pseudocode ✅ COMPLETE
- [x] ArrayIteratorParser algorithms → `pseudocode/PSEUDOCODE.md`
  - [x] Object iteration algorithm
  - [x] Array iteration algorithm
  - [x] Field extraction with JSONPath
  - [x] Timestamp parsing logic
- [x] Parser integration algorithms
  - [x] HttpJsonSource integration
  - [x] TimescaleSource integration
- [x] Stream migration algorithms
  - [x] Existing stream migration steps
  - [x] NWS stream addition steps

### A - Architecture ✅ COMPLETE
- [x] Parser system unification → `architecture/ARCHITECTURE.md`
  - [x] Parser trait as single source of truth
  - [x] ArrayIteratorParser implementation design
  - [x] Source integration patterns (dependency injection)
  - [x] ResponseParser deprecation plan
- [x] Stream inventory and migration plan
  - [x] All 5 streams documented (3 existing + 2 new NWS)
  - [x] Migration sequence defined
  - [x] Rollback strategy documented
- [x] Configuration management
  - [x] Array iterator YAML schema
  - [x] Timestamp extraction configuration
  - [x] NWS API endpoint configuration

### R - Refinement ⏳ IN PROGRESS
**Phase Start**: 2025-12-21 21:00 UTC

#### Implementation Components (FR-007 through FR-013)

**1. ArrayIteratorParser Implementation (FR-007)**
- [x] Create `core/src/parsers/array_iterator.rs` ✅ DONE
  - [x] Implementation complete with all features
  - [x] String parsing with regex support
  - [x] Enum mapping for cardinal directions
  - [x] Metadata tag extraction
  - [x] Timestamp extraction per element
  - [ ] ⚠️ **FIX TESTS**: Update test code to use new constructor signature
- [x] Update `core/src/parsers/config.rs` ✅ DONE
  - [x] Added `array_config: Option<ArrayIteratorConfig>` field
  - [x] ParserType::ArrayIterator enum variant
- [x] Update `core/src/parsers/factory.rs` ✅ DONE
  - [x] Handles `ParserType::ArrayIterator` case
  - [x] Creates ArrayIteratorParser from config
- [x] Update `core/src/parsers/mod.rs` ✅ DONE
  - [x] Exports ArrayIteratorParser and related types

**2. Response Timestamp Extraction (FR-008)**
- [ ] Extend ParserConfig
  - [ ] Add `timestamp_field: Option<String>`
  - [ ] Add `timestamp_format: Option<TimestampFormat>`
- [ ] Implement timestamp extraction in parsers
  - [ ] JsonPathParser: extract timestamp from response
  - [ ] ArrayIteratorParser: extract per-element timestamps
  - [ ] Fallback to poll time if missing/invalid
- [ ] Write unit tests for timestamp extraction
  - [ ] ISO8601 parsing
  - [ ] Unix epoch parsing
  - [ ] Fallback behavior

**3. Response Metadata Tags (FR-009)**
- [ ] Extend ParserConfig
  - [ ] Add `metadata_tags: Option<Vec<MetadataTagConfig>>`
- [ ] Implement metadata extraction
  - [ ] Extract from response root before iteration
  - [ ] Add to all generated TimeSeriesPoints
- [ ] Write unit tests for metadata propagation

**4. String Value Parsing (FR-010)**
- [ ] Create `core/src/parsers/string_parser.rs`
  - [ ] Implement regex value extraction
  - [ ] Support integer and float extraction
  - [ ] Handle parse failures gracefully
- [ ] Write unit tests
  - [ ] "10 mph" → 10.0
  - [ ] "12.5 mph" → 12.5
  - [ ] "10 to 20 mph" → 10.0 (first number)
  - [ ] "Variable" → None (with warning)

**5. Enum Mapping (FR-011)**
- [ ] Create `core/src/parsers/enum_mapper.rs`
  - [ ] Implement cardinal direction mapper
  - [ ] Implement custom enum mapper
  - [ ] Case-insensitive matching
- [ ] Write unit tests
  - [ ] All 16 cardinal directions
  - [ ] Case insensitivity
  - [ ] Unknown values → None

**6. Parser/Source Integration (FR-012)**
- [ ] Update GenericHttpPollingSource
  - [ ] Change from ResponseParser to Parser trait
  - [ ] Update constructor to accept `Box<dyn Parser>`
  - [ ] Update `poll_endpoint()` to use parser.parse()
  - [ ] Update unit tests
- [ ] Update SourceManager
  - [ ] Add `create_parser_from_config()` method
  - [ ] Inject Parser into sources
  - [ ] Update stream spawning logic
- [ ] Integration tests
  - [ ] Test GenericHttpPollingSource with JsonPathParser
  - [ ] Test GenericHttpPollingSource with ArrayIteratorParser
  - [ ] Verify existing streams continue working

**7. Stream Configuration**
- [ ] Create `config/base/streams/nws-observations.yaml`
  - [ ] Configure JsonPathParser
  - [ ] Set timestamp_field to extract observation time
  - [ ] Define field_mappings for all metrics
- [ ] Create `config/base/streams/nws-forecast-hourly.yaml`
  - [ ] Configure ArrayIteratorParser
  - [ ] Set array_path to "properties.periods"
  - [ ] Configure metadata_tags for issue_time
  - [ ] Define field_mappings with string_parse and enum_map
- [ ] Migrate existing stream configs (if needed)
  - [ ] outdoor-weather.yaml
  - [ ] outdoor-air-quality.yaml

**8. Legacy Removal (FR-013)**
- [ ] Delete deprecated code
  - [ ] `core/src/sources/parsers/weather.rs`
  - [ ] `core/src/sources/parsers/air_pollution.rs`
  - [ ] `core/src/sources/parsers/mod.rs`
  - [ ] ResponseParser trait (if fully replaced)
- [ ] Update documentation
  - [ ] Remove references to deleted parsers
  - [ ] Update architecture diagrams
- [ ] Verify no remaining references
  - [ ] `rg "ResponseParser" --type rust`
  - [ ] `rg "WeatherParser" --type rust`
  - [ ] `rg "AirPollutionParser" --type rust`

#### Acceptance Criteria Tracking

**Array Iteration (FR-007)**
- [ ] AC-007: Config with `array_path` iterates over JSON arrays
- [ ] AC-008: Each array element produces N points (one per mapping)
- [ ] AC-009: NWS forecast with 156 periods produces 936 points (6 metrics)
- [ ] AC-010: Empty arrays produce zero points without error
- [ ] AC-011: Array iteration works with nested paths

**Response Timestamp Extraction (FR-008)**
- [ ] AC-013: Config `timestamp_field` extracts timestamp from response
- [ ] AC-014: Parser parses ISO8601 timestamps correctly
- [ ] AC-015: NWS observations use observation timestamp, not poll time
- [ ] AC-016: NWS forecasts use `startTime` from each period
- [ ] AC-017: Invalid timestamps fall back to `Utc::now()` with warning

**Response Metadata Tags (FR-009)**
- [ ] AC-019: Config `metadata_tags` extracts fields from response root
- [ ] AC-020: Metadata tags applied to ALL points from response
- [ ] AC-021: NWS forecast `issue_time` tag present on all 936 points
- [ ] AC-022: JSONPath expressions work for metadata tags

**String Value Parsing (FR-010)**
- [ ] AC-024: Config `string_parse` with regex pattern extracts numbers
- [ ] AC-025: "15 mph" → 15.0
- [ ] AC-026: "12.5 mph" → 12.5
- [ ] AC-027: "10 to 20 mph" → 10.0 (first number)
- [ ] AC-028: "Variable" → null with warning

**Enum Mapping (FR-011)**
- [ ] AC-030: Config `enum_map` maps strings to numbers
- [ ] AC-031: "NE" → 45.0 (wind direction mapping)
- [ ] AC-032: Unknown values logged as warnings, field skipped
- [ ] AC-033: Case-insensitive matching supported

**Parser/Source Integration (FR-012)**
- [ ] AC-035: GenericHttpPollingSource uses Parser trait
- [ ] AC-036: ResponseParser trait removed from HTTP sources
- [ ] AC-037: SourceManager creates Parser from config
- [ ] AC-038: Parser injected via constructor into sources

**NWS Streams**
- [ ] AC-040: `nws-observations` stream configured in etcd
- [ ] AC-041: NWS observations polling every 10 minutes
- [ ] AC-042: Observation timestamp extracted from response
- [ ] AC-043: All observation fields extracted correctly
- [ ] AC-044: `nws-forecast-hourly` stream configured in etcd
- [ ] AC-045: NWS forecast polling every 10 minutes
- [ ] AC-046: 156 forecast periods produce 936 points
- [ ] AC-047: `issue_time` tag present on all forecast points
- [ ] AC-048: Wind speed string parsing works
- [ ] AC-049: Wind direction enum mapping works
- [ ] AC-050: Forecast timestamps use `startTime`, not poll time

**Backward Compatibility**
- [ ] AC-051: Existing `air-quality` stream works unchanged
- [ ] AC-052: Existing `outdoor-weather` stream works unchanged
- [ ] AC-053: Existing `outdoor-air-quality` stream works unchanged
- [ ] AC-054: All existing integration tests pass

#### Testing Checklist
- [ ] Unit tests for ArrayIteratorParser
- [ ] Unit tests for timestamp extraction
- [ ] Unit tests for string parsing
- [ ] Unit tests for enum mapping
- [ ] Integration tests for GenericHttpPollingSource
- [ ] End-to-end tests with mock NWS API
- [ ] Regression tests for existing streams
- [ ] Performance benchmarks (parsing <1ms per message)

### C - Completion ⏳ PENDING
- [ ] All 5 streams operational in Bronze layer
- [ ] Integration tests passing
- [ ] Legacy code removed
- [ ] Documentation updated
- [ ] Deployment verified
- [ ] Performance validated

## Progress Summary

### ✅ Completed (SPARC S, P, A Phases)

**Specification Phase**
- [x] Feature scope expanded from NWS integration to comprehensive Bronze layer enhancement
- [x] BUG-002 requirements fully analyzed
- [x] All 5 stream configurations documented
- [x] NWS API endpoints validated (KSGJ observations, JAX forecast)
- [x] Complete requirements specification created

**Pseudocode Phase**
- [x] ArrayIteratorParser algorithms designed
- [x] Object and array iteration logic documented
- [x] Timestamp extraction pseudocode
- [x] Source integration algorithms
- [x] Stream migration procedures

**Architecture Phase**
- [x] Parser system unification design
- [x] ArrayIteratorParser component architecture
- [x] Source integration patterns (dependency injection)
- [x] Migration strategy with rollback plan
- [x] Configuration schema for array iteration
- [x] ResponseParser deprecation plan

**Key Architectural Decisions**
| Decision | Choice | Rationale |
|----------|--------|-----------|
| Parser Unification | Single Parser trait | Eliminate ResponseParser duplication |
| Array Iteration | New `array_iterator` parser_type | NWS forecast periods require iteration |
| Timestamp Handling | Extract from response data | Forecast validity times needed for accuracy |
| Source Integration | Parser as trait object in sources | Dependency injection for flexibility |
| Scope Limitation | Bronze layer only | Silver layer ETL is separate concern (dp-XXX) |

### 🔄 Next Phase: Refinement (TDD Implementation)

**Ready to implement**:
1. ArrayIteratorParser with unit tests
2. Parser integration into HttpJsonSource and TimescaleSource
3. Migration of 3 existing streams
4. Addition of 2 NWS streams
5. Deletion of legacy ResponseParser code

**Waiting on**: Human approval to proceed with implementation

### ⏳ Future: Completion Phase
- Verify all 5 streams operational
- Validate performance targets
- Complete integration testing
- Deploy to production
- Update platform documentation

## Active Work

**Current Status**: ArrayIteratorParser implementation COMPLETE, but test code needs updating.

**Build Status**: ✅ PASSING (`cargo check -p platform-core` succeeds)
**Test Status**: ❌ FAILING (test code uses old constructor signature)

**Next Action**: Fix test code to match new ParserConfig structure

**Immediate Tasks for ndp-rust-dev**:
1. ⚠️ **URGENT**: Fix test code in `array_iterator.rs` (lines 405-427, 630+)
   - Update `create_test_parser()` to use new single-argument constructor
   - Add `array_config` field to `ParserConfig` initializers in tests
2. Run `cargo test -p platform-core parsers::array_iterator` to verify fixes
3. Verify all unit tests pass before proceeding
4. Begin NWS stream configuration testing with real API payloads

## Blockers

### CRITICAL: Test Code Errors (Build Passing, Tests Failing)

**Status**: Build succeeds (`cargo check -p platform-core` passes with warnings only), but test compilation fails.

**Root Cause**: Tests in `array_iterator.rs` use old constructor signature:
- Tests call `ArrayIteratorParser::from_config(base_config, array_config)` (2 args)
- Actual implementation expects `from_config(config: ParserConfig)` (1 arg)
- `ParserConfig` now has embedded `array_config: Option<ArrayIteratorConfig>` field

**Affected Test Functions**:
1. `create_test_parser()` helper (line 405-427)
2. `test_array_iteration_produces_correct_point_count()` (line 630+)

**Error Messages**:
```
error[E0061]: this function takes 1 argument but 2 arguments were supplied
   --> core/src/parsers/array_iterator.rs:426:9
426 |         ArrayIteratorParser::from_config(base_config, array_config).unwrap()
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^              ------------ unexpected argument #2

error[E0063]: missing field `array_config` in initializer of `config::ParserConfig`
   --> core/src/parsers/array_iterator.rs:630:27
630 |         let base_config = ParserConfig {
    |                           ^^^^^^^^^^^^ missing `array_config`
```

**Files Requiring Fixes**: `/workspaces/neural-data-platform/core/src/parsers/array_iterator.rs`

**Impact**: Integration tests cannot run, blocking verification of ArrayIteratorParser implementation.

## Dependencies

**Upstream (All exist)**:
- ✅ Parser trait (`neural-core/src/common/parser.rs`)
- ✅ GenericHttpPollingSource (from air-005)
- ✅ IngestionCoordinator and stream routing
- ✅ ParquetStore implementation
- ✅ etcd configuration infrastructure
- ✅ TimescaleSource implementation

**Related**:
- 🔗 BUG-002: Config-driven parsing bug (integrated into this feature)
- 📝 STREAM-CONFIGS.md: All 5 stream configurations documented

## Technical Decisions

### Key Decisions (Architecture Phase)

| Date | Decision | Rationale | Status |
|------|----------|-----------|--------|
| 2025-12-21 | Unify on Parser trait | Remove ResponseParser duplication, single source of truth | ✅ Approved |
| 2025-12-21 | Add array_iterator parser_type | NWS forecast periods are JSON arrays requiring iteration | ✅ Approved |
| 2025-12-21 | Extract timestamps from response | Forecast validity requires actual issue_time and forecast_valid_time | ✅ Approved |
| 2025-12-21 | Tall format for forecast data | Standard time-series format, simplifies Silver layer queries | ✅ Approved |
| 2025-12-21 | Bronze layer scope only | Silver layer ETL is separate feature (dp-XXX) | ✅ Approved |
| 2025-12-21 | Station: KSGJ | Nearest NWS station to St. Augustine (~5 miles) | ✅ Approved |
| 2025-12-21 | Grid point: JAX/79,49 | Jacksonville WFO coverage for St. Augustine area | ✅ Approved |
| 2025-12-21 | Poll interval: 10 minutes | Consistent with existing streams, conservative for public API | ✅ Approved |

### Implementation Decisions (Refinement Phase)

*(To be filled during TDD implementation)*

## Bugs

| ID | Status | Summary | Integrated |
|----|--------|---------|------------|
| BUG-002 | Integrated | Config-driven parsing incomplete | ✅ Into air-006 specification |

**Note**: BUG-002 evolved into the core of this feature. The bug fix (completing config-driven parsing) is now part of the ArrayIteratorParser implementation in the Refinement phase.

## Branch

**Strategy**: Trunk-Based Development (per `ndp-github-workflow` skill)
- Commit directly to `main` with conventional commit messages
- Feature branch optional for large changes
- Use `feat(air-006):` prefix for all commits

## Stream Inventory

Complete list of streams affected by this feature:

| Stream ID | Type | Parser Type | Current Status | Target Status |
|-----------|------|-------------|----------------|---------------|
| `airnow-observations` | Existing | Uses ResponseParser | ⚠️ Needs migration | Use Parser trait with `object` |
| `openmeteo-current` | Existing | Uses ResponseParser | ⚠️ Needs migration | Use Parser trait with `object` |
| `timescale-bronze-etl` | Existing | Uses ResponseParser | ⚠️ Needs migration | Use Parser trait with `object` |
| `nws-observations-ksgj` | New | N/A | 🆕 To be added | Use Parser trait with `object` |
| `nws-forecast-jax` | New | N/A | 🆕 To be added | Use Parser trait with `array_iterator` |

## Related Documents

**Feature Documentation**:
- `SCOPE.md` - Expanded feature scope
- `specification/SPECIFICATION.md` - Complete requirements
- `specification/BUG-002-SPECIFICATION.md` - Config-driven parsing fix
- `specification/STREAM-CONFIGS.md` - All 5 stream configurations
- `pseudocode/PSEUDOCODE.md` - ArrayIteratorParser algorithms
- `architecture/ARCHITECTURE.md` - Parser unification design

**Related Features**:
- `bugs/BUG-002-config-driven-parsing.md` - Original bug report (integrated)
- `../air-005/` - OpenWeatherMap integration (similar HTTP polling pattern)
- `../dp-XXX/` - Future Silver layer feature (will consume Bronze data)

**Platform Documentation**:
- `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` - System architecture
- `docs/procedures/HOW_TO_ADD_NEW_STREAM.md` - Stream addition procedure
- `.claude/agents/ndp/README.md` - NDP agent roster

## Next Steps

### Phase 1: Refinement (TDD Implementation) - READY TO START

**Step 1: ArrayIteratorParser Implementation**
```bash
# TDD Cycle
1. Write failing test for object iteration
2. Implement object iteration in ArrayIteratorParser
3. Write failing test for array iteration
4. Implement array iteration
5. Write failing test for timestamp extraction
6. Implement timestamp extraction
7. Refactor and optimize
```

**Step 2: Source Integration**
```bash
# Update HttpJsonSource to use Parser
1. Replace ResponseParser with Parser trait object
2. Update unit tests
3. Verify existing streams still work

# Update TimescaleSource similarly
```

**Step 3: Stream Migration**
```bash
# Migrate existing streams one by one
1. airnow-observations (low risk, well-tested)
2. openmeteo-current (low risk, well-tested)
3. timescale-bronze-etl (medium risk, test carefully)
```

**Step 4: Add NWS Streams**
```bash
# Add new streams
1. nws-observations-ksgj (standard object iteration)
2. nws-forecast-jax (new array iteration feature)
```

**Step 5: Legacy Cleanup**
```bash
# Delete deprecated code
1. Remove ResponseParser trait
2. Remove old parser implementations
3. Update documentation
```

### Phase 2: Completion - AFTER IMPLEMENTATION

1. ✅ All integration tests passing
2. 📊 Performance validation (no regression)
3. 📝 Documentation updates
4. 🚀 Production deployment
5. ✅ Verification of all 5 streams operational

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Ingestion latency | <100ms per record | No regression from current performance |
| Array iteration overhead | <50ms for 100 periods | NWS forecast typically has ~156 periods |
| Memory usage | <10MB per stream | Parser should be lightweight |
| Parquet write latency | <500ms per batch | Consistent with existing streams |
| Configuration load time | <100ms | etcd lookup and Parser initialization |

## Implementation Coordination

### Implementation Strategy

**Approach**: Test-Driven Development (TDD)
1. Write failing tests first for each component
2. Implement minimum code to pass tests
3. Refactor and optimize
4. Integration tests after unit tests pass
5. End-to-end verification with real API

**Recommended Implementation Order**:
1. **String parsing & enum mapping** (FR-010, FR-011) - Foundational utilities
2. **Timestamp extraction** (FR-008) - Core feature
3. **Metadata tags** (FR-009) - Build on timestamp extraction
4. **ArrayIteratorParser** (FR-007) - Uses all above features
5. **Source integration** (FR-012) - Wiring into GenericHttpPollingSource
6. **Stream configs** - YAML configuration files
7. **Legacy removal** (FR-013) - Cleanup after verification

### Next Actions

**Immediate (Ready to Start)**:
1. **ndp-rust-dev**: Implement string_parser.rs with unit tests
2. **ndp-rust-dev**: Implement enum_mapper.rs with unit tests
3. **ndp-tester**: Prepare integration test fixtures (mock NWS responses)

**Waiting On**: Human approval to begin implementation

**Blockers**: None

## Recent Activity

| Date | Activity | Agent |
|------|----------|-------|
| 2025-12-22 18:30 | STATUS assessment: Build passing, test code needs fixes | ndp-scrum-master |
| 2025-12-22 18:30 | Identified blocker: Test constructor signatures mismatch | ndp-scrum-master |
| 2025-12-22 18:30 | Verified NWS stream configs exist and are properly formatted | ndp-scrum-master |
| 2025-12-21 ~20:00 | ArrayIteratorParser fully implemented with all features | ndp-rust-dev |
| 2025-12-21 ~20:00 | NWS stream configs created (observations + forecast) | ndp-rust-dev |
| 2025-12-21 21:00 | Refinement phase tracking added to STATUS.md | ndp-scrum-master |
| 2025-12-21 21:00 | Implementation components and acceptance criteria organized | ndp-scrum-master |
| 2025-12-21 20:30 | STATUS.md updated with comprehensive tracking | ndp-scrum-master |
| 2025-12-21 19:15 | Architecture phase completed | ndp-architect |
| 2025-12-21 19:10 | Pseudocode phase completed | ndp-architect |
| 2025-12-21 19:05 | Specification phase completed | ndp-architect |
| 2025-12-21 19:00 | Scope expanded to include BUG-002 and parser unification | ndp-scrum-master |
| 2025-12-21 18:59 | Feature directory structure initialized | System |
| 2025-12-21 18:00 | Research completed: NWS API, KSGJ validation, data modeling | Human + ndp-architect |

## Notes

**Pattern Compliance**:
- ✅ Following NDP feature lifecycle (SPARC methodology)
- ✅ Using established Domain Adapter patterns
- ✅ Leveraging existing GenericHttpPollingSource (DRY principle)
- ✅ Configuration-driven approach (etcd stream configs)

**Key Design Principles**:
- **Single Responsibility**: Parser unification removes ResponseParser duplication
- **Dependency Injection**: Sources accept Parser as trait object for flexibility
- **Test-Driven Development**: TDD approach in Refinement phase
- **Backward Compatibility**: Existing streams must continue working during migration
- **Incremental Migration**: One stream at a time with rollback capability

**Risk Mitigation**:
- Comprehensive unit tests before integration
- Migrate low-risk streams first (airnow, openmeteo)
- Keep ResponseParser until all streams migrated
- Rollback plan documented in architecture

**Future Work** (out of scope for air-006):
- Silver layer ETL (dp-XXX feature)
- Forecast verification queries
- ML feature engineering from forecast data
- Dashboard visualization of forecasts
