# AIR-004 Swarm Implementation - Kickoff Report

**SwarmLead Coordinator**: Active
**Date**: 2025-12-15
**Session**: AIR-004 Multi-Stream Platform Implementation

---

## Situation Analysis

### Current State Assessment

**Foundation Complete (40%)**:
- ✅ Core types implemented:
  - `/workspaces/neural-data-platform/core/src/types/stream_record.rs` (316 lines, 100% tested)
  - `/workspaces/neural-data-platform/core/src/types/stream_config.rs` (645 lines, 100% tested)
- ✅ Stream registry wrapper:
  - `/workspaces/neural-data-platform/config-client/src/stream/registry.rs` (367 lines with tests)
- ✅ Architecture documentation:
  - ADR-001-MULTISTREAM-FOUNDATION.md
  - ADR-002-STREAM-REGISTRY.md
  - Complete test strategy (London TDD)

**Critical Blockers Identified**:
1. **5 API Route Tests FAILING** (404 errors) - BLOCKING
2. **IngestionCoordinator NOT IMPLEMENTED** - Core feature missing
3. **HttpPoller NOT INTEGRATED** - Code exists but not wired
4. **WebhookHandler NOT IMPLEMENTED** - Required for webhook support
5. **Production unwrap() usage** - 146+ instances causing crash risk

### Test Failure Analysis

**Failing Tests** (5/59):
```
test api::routes::tests::test_aggregate_endpoint_mean ... FAILED
test api::routes::tests::test_alerts_endpoint ... FAILED
test api::routes::tests::test_forecast_endpoint ... FAILED
test api::routes::tests::test_latest_readings_endpoint_with_data ... FAILED
test api::routes::tests::test_readings_time_range_query ... FAILED
```

**Passing Tests** (54/59):
- Health endpoint working
- Locations endpoint working
- CORS headers working
- Not found route working
- Invalid query params working

**Root Cause Hypothesis**:
Routes are defined in `create_router()` at `/workspaces/neural-data-platform/apps/air-quality-app/src/api/routes.rs` but something in the routing configuration or test server setup is causing 404s for specific endpoints.

### Review Report Key Findings

From `/workspaces/neural-data-platform/product/features/air-004/REVIEW_REPORT.md`:

**CRITICAL-001**: Multi-stream functionality NOT IMPLEMENTED
- No generic ingestion coordinator
- etcd stream configuration paths not implemented
- Stream-isolated storage structure missing

**CRITICAL-002**: 5 integration tests FAILING
- Routes not properly registered or paths changed
- Blocks deployment and further development

**CRITICAL-003**: Excessive unwrap() in production
- 146+ instances across codebase
- Will cause panic crashes in production

---

## Implementation Strategy

### Hierarchical Swarm Approach

**Coordination Model**: Single SwarmLead → Specialist Agents

```
    👑 SWARMLEAD (Coordinator)
   /    |    |    \
  🔧   💻   🧪   📦
 FIX  CODE TEST PKG
```

### Phased Execution Plan

#### Phase 1: UNBLOCK (Priority: P0)
**Duration**: 2-4 hours
**Agent**: Route Fixer (backend-dev)
**Goal**: Get all 59 tests passing
**Success Metric**: 0 failing tests

#### Phase 2: COORDINATE (Priority: P1)
**Duration**: 8-12 hours
**Agent**: Coordinator Implementer (backend-dev + architect)
**Goal**: Multi-stream coordinator operational
**Success Metric**: 2+ streams ingesting data

#### Phase 3: INTEGRATE (Priority: P1)
**Duration**: 6-10 hours
**Agent**: Source Integration Specialist (backend-dev)
**Goal**: HTTP polling and webhook sources working
**Success Metric**: All 3 source types operational

#### Phase 4: HARDEN (Priority: P2)
**Duration**: 4-6 hours
**Agent**: Error Handling Auditor (reviewer)
**Goal**: Replace production unwrap() calls
**Success Metric**: <20 production unwrap() instances

#### Phase 5: VALIDATE (Priority: P1)
**Duration**: 4-6 hours
**Agent**: Integration Tester (tester)
**Goal**: Full pipeline validation
**Success Metric**: 90% coverage, backward compatibility verified

---

## Agent Spawn Plan

### Agent 1: Route Fixer (IMMEDIATE)

**Type**: Backend Developer
**Specialization**: API routing, Axum, testing
**Priority**: CRITICAL BLOCKER

**Task Brief**:
```
OBJECTIVE: Fix 5 failing API route tests in air-quality-app

FAILING TESTS:
1. test_aggregate_endpoint_mean - GET /api/v1/aggregate
2. test_alerts_endpoint - GET /api/v1/alerts
3. test_forecast_endpoint - GET /api/v1/forecast
4. test_latest_readings_endpoint_with_data - GET /api/v1/readings/latest
5. test_readings_time_range_query - GET /api/v1/readings

CONSTRAINTS:
- Do NOT break 54 passing tests
- Do NOT modify API contracts
- Use existing mock patterns from routes.rs

DELIVERABLES:
1. Fixed routes.rs (if needed)
2. Fixed handler imports (if needed)
3. Test validation report
4. Memory update: swarm/agent/route-fixer/status

SUCCESS CRITERIA:
✅ All 59 tests passing
✅ cargo test --package air-quality-app --lib passes
✅ No new warnings
```

**Expected Duration**: 2-4 hours
**Dependencies**: None
**Blocker Resolution**: This MUST complete before other agents can proceed safely

### Agent 2: Coordinator Implementer (QUEUED)

**Type**: Backend Developer + Architect
**Specialization**: Async Rust, tokio, system design
**Priority**: HIGH (after Agent 1)

**Task Brief**:
```
OBJECTIVE: Implement IngestionCoordinator + SourceManager + IngestionRouter

DELIVERABLES:
1. apps/air-quality-app/src/coordinator/ingestion_coordinator.rs
2. apps/air-quality-app/src/coordinator/source_manager.rs
3. apps/air-quality-app/src/coordinator/router.rs
4. apps/air-quality-app/src/coordinator/mod.rs
5. London TDD tests for all components (90% coverage)

ARCHITECTURE CONSTRAINTS:
- Use existing StreamRegistry from config-client
- Follow MqttHandler pattern for consistency
- Support dynamic stream addition via etcd watch
- Implement graceful shutdown

SUCCESS CRITERIA:
✅ Can spawn sources for 2+ streams
✅ Routes points to correct storage writers
✅ Integration test demonstrates multi-stream
✅ All baseline tests still passing
```

**Expected Duration**: 8-12 hours
**Dependencies**: Agent 1 complete
**Reference**: IMPLEMENTATION_PLAN.md Phase 4

### Agent 3: Source Integration Specialist (QUEUED)

**Type**: Backend Developer
**Specialization**: HTTP clients, webhooks, async
**Priority**: HIGH (parallel with Agent 2)

**Task Brief**:
```
OBJECTIVE: Integrate HttpPoller and implement WebhookHandler

EXISTING CODE TO INTEGRATE:
- core/src/sources/http_poll.rs (exists, needs wiring)

NEW IMPLEMENTATIONS:
1. apps/air-quality-app/src/sources/webhook_handler.rs
2. apps/air-quality-app/src/sources/mqtt_wrapper.rs
3. apps/air-quality-app/src/sources/mod.rs

TECHNICAL REQUIREMENTS:
- HttpPoller: Use reqwest, implement retry with exponential backoff
- WebhookHandler: Axum server, Bearer auth, rate limiting (1000 req/min)
- All sources: Health check endpoints, channel-based forwarding

SUCCESS CRITERIA:
✅ HttpPoller integrated with tests (wiremock)
✅ WebhookHandler accepts POST /api/streams/{id}/events
✅ Health checks operational for all sources
✅ London TDD tests with mocked dependencies
```

**Expected Duration**: 6-10 hours
**Dependencies**: Can start after Agent 1, parallel to Agent 2
**Reference**: IMPLEMENTATION_PLAN.md Phase 3

### Agent 4: Error Handling Auditor (QUEUED)

**Type**: Code Reviewer + Refactoring Specialist
**Specialization**: Error handling, production quality
**Priority**: MEDIUM (parallel work)

**Task Brief**:
```
OBJECTIVE: Replace production unwrap() calls with proper error handling

SCOPE:
- core/src/storage/parquet.rs
- core/src/sources/**/*.rs
- apps/air-quality-app/src/**/*.rs (non-test code)

REPLACEMENT STRATEGY:
- Use ? operator where possible
- Use map_err() for context
- Use expect() with meaningful messages as last resort
- Leave test code unwrap() unchanged

TARGET:
- Reduce production unwrap() count by >80%
- Current: 146+ instances
- Target: <20 instances

SUCCESS CRITERIA:
✅ Production unwrap() count <20
✅ All tests still passing
✅ No new panic paths introduced
✅ Error messages provide actionable context
```

**Expected Duration**: 4-6 hours
**Dependencies**: Can work in parallel
**Reference**: REVIEW_REPORT.md CRITICAL-003

### Agent 5: Integration Tester (QUEUED)

**Type**: Test Engineer
**Specialization**: Integration testing, TDD, quality gates
**Priority**: HIGH (after Agent 2 & 3)

**Task Brief**:
```
OBJECTIVE: Create integration tests for multi-stream pipeline

DELIVERABLES:
1. apps/air-quality-app/tests/multi_stream_integration_test.rs
2. Backward compatibility test with AIR-002 Parquet files
3. Performance regression validation
4. Integration test report

TEST SCENARIOS:
- 2+ streams ingesting simultaneously
- Dynamic stream addition via etcd
- Source failure and recovery
- Cross-stream isolation
- AIR-002/003 data format compatibility

SUCCESS CRITERIA:
✅ 90% coverage for new coordinator code
✅ All baseline AIR-002/003 tests passing
✅ Performance regression <10%
✅ Memory usage within Pi limits (<896MB)
```

**Expected Duration**: 4-6 hours
**Dependencies**: Agent 2 and 3 near completion
**Reference**: TEST_STRATEGY.md

---

## Quality Gates

### Gate 1: Route Fix (End of Phase 1)
**Criteria**:
- ✅ All 59 tests passing (cargo test --package air-quality-app --lib)
- ✅ No new compiler warnings
- ✅ Test report generated

**Go/No-Go Decision**: Required before spawning Agent 2 & 3

### Gate 2: Coordinator Implementation (End of Phase 2)
**Criteria**:
- ✅ IngestionCoordinator operational with 2+ streams
- ✅ London TDD tests passing (90% coverage)
- ✅ Integration test demonstrates multi-stream ingestion
- ✅ All baseline tests still passing

**Go/No-Go Decision**: Required before final integration

### Gate 3: Source Integration (End of Phase 3)
**Criteria**:
- ✅ HttpPoller integrated and tested
- ✅ WebhookHandler operational with auth
- ✅ Health checks working for all sources
- ✅ Integration tests passing

**Go/No-Go Decision**: Required before deployment prep

### Gate 4: Production Ready (End of Phase 5)
**Criteria**:
- ✅ All integration tests passing
- ✅ Backward compatibility verified (AIR-002/003)
- ✅ Performance regression <10%
- ✅ Production unwrap() <20 instances
- ✅ Memory usage <896MB on Pi
- ✅ Documentation complete

**Go/No-Go Decision**: Production deployment authorization

---

## Risk Management

### High-Risk Items

**Risk 1: Test Cascades**
- **Impact**: Fixes to failing tests break passing tests
- **Mitigation**: Run full suite after each change, use git branches
- **Owner**: Agent 1 (Route Fixer)

**Risk 2: Coordinator Complexity**
- **Impact**: Implementation takes >12 hours, misses timeline
- **Mitigation**: Break into smaller components, test incrementally
- **Owner**: Agent 2 (Coordinator Implementer)

**Risk 3: Integration Incompatibilities**
- **Impact**: Sources don't integrate smoothly with coordinator
- **Mitigation**: Follow existing MqttHandler pattern strictly
- **Owner**: Agent 3 (Source Specialist)

### Medium-Risk Items

**Risk 4: Performance Regression**
- **Impact**: Multi-stream overhead slows system >10%
- **Mitigation**: Benchmark at each gate, optimize before proceeding
- **Owner**: Agent 5 (Integration Tester)

**Risk 5: Memory Limit on Pi**
- **Impact**: Exceeds 896MB budget, causes OOM kills
- **Mitigation**: Monitor during testing, optimize batch sizes
- **Owner**: SwarmLead (Coordinator)

---

## Communication Protocols

### Memory Store Updates

**Frequency**: Every 2 hours
**Format**: JSON status to swarm memory

**Key Structure**:
```
swarm/
├── coordinator/status              # SwarmLead updates
├── coordinator/blockers            # Critical issues
├── coordinator/decisions           # Architectural decisions
├── agent/route-fixer/status        # Agent 1 progress
├── agent/coordinator-impl/status   # Agent 2 progress
├── agent/source-specialist/status  # Agent 3 progress
├── agent/error-handler/status      # Agent 4 progress
└── agent/integration-tester/status # Agent 5 progress
```

### Blocker Escalation

**P0 (CRITICAL)**: Blocks all work
- **Action**: Immediate escalation to SwarmLead
- **Response Time**: <30 minutes

**P1 (HIGH)**: Blocks specific agent
- **Action**: Report to SwarmLead within 2 hours
- **Response Time**: <2 hours

**P2 (MEDIUM)**: Slows progress
- **Action**: Report at next sync (4 hours)
- **Response Time**: <4 hours

### Status Reports

**Frequency**: Every 6 hours
**Location**: `/workspaces/neural-data-platform/product/features/air-004/SWARM_STATUS.md`

**Format**:
```markdown
# Swarm Status Report - [Timestamp]

## Overall Progress: [X]%

### Phase 1: Route Fix - [Status]
- Agent 1: [Progress details]
- Blockers: [List]

### Phase 2: Coordinator - [Status]
- Agent 2: [Progress details]
- Blockers: [List]

[... etc ...]
```

---

## Success Metrics

### Code Quality
- All tests passing (59/59 → target)
- Test coverage >90% for new code
- Production unwrap() <20 instances
- No new compiler warnings

### Functionality
- Multi-stream coordinator operational
- 3+ source types working (MQTT, HTTP, Webhook)
- Stream isolation verified
- Dynamic stream addition working

### Performance
- Ingestion throughput: >1 msg/sec per stream
- Config read latency: <10ms
- API response time: <200ms p95
- Memory usage: <896MB on Pi

### Backward Compatibility
- Existing air-quality stream working
- AIR-002/003 Parquet files readable
- Existing API endpoints unchanged
- No breaking config changes

---

## Next Actions (Immediate)

### Step 1: Analyze Route Failures
- SwarmLead examines test output details
- Identify exact failure modes (404 errors)
- Prepare detailed brief for Agent 1

### Step 2: Spawn Agent 1 (Route Fixer)
- Create detailed task specification
- Provide test failure context
- Set success criteria and timeline
- Monitor progress via memory store

### Step 3: Prepare Agent 2 Brief
- While Agent 1 works, prepare coordinator design
- Review IMPLEMENTATION_PLAN.md Phase 4 details
- Identify integration points
- Draft component interfaces

### Step 4: Monitor and Coordinate
- Check Agent 1 status every 2 hours
- Unblock any issues immediately
- Validate Gate 1 completion
- Spawn Agent 2 & 3 when ready

---

## Timeline

**Phase 1**: 2-4 hours (Route fix)
**Phase 2**: 8-12 hours (Coordinator implementation)
**Phase 3**: 6-10 hours (Source integration, parallel)
**Phase 4**: 4-6 hours (Error handling, parallel)
**Phase 5**: 4-6 hours (Integration testing)

**Total Estimated**: 24-38 hours (3-5 days)

**Critical Path**: Phase 1 → Phase 2 → Phase 5

---

## Deliverables

### Code Artifacts
1. Fixed routes and tests
2. IngestionCoordinator + SourceManager + Router
3. HttpPoller integration + WebhookHandler
4. Improved error handling
5. Integration test suite

### Documentation
1. Test fix report
2. Implementation completion report
3. Integration test results
4. Updated architecture docs (if needed)
5. Deployment guide for Pi

### Quality Assurance
1. All 59+ tests passing
2. 90% coverage for new code
3. Performance validation report
4. Backward compatibility verification

---

**Status**: SWARM READY TO DEPLOY
**First Agent**: Route Fixer (backend-dev)
**Next Action**: Detailed test failure analysis + Agent 1 spawn
**Confidence**: HIGH (clear blockers, solid foundation, detailed plan)

---

*Swarm Kickoff Report v1.0*
*Generated by SwarmLead Coordinator*
*Session Start: 2025-12-15*
