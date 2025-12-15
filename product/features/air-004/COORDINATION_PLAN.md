# AIR-004 SwarmLead Coordination Plan

**Coordinator**: SwarmLead Agent
**Date**: 2025-12-15
**Feature**: Multi-Stream Data Platform Implementation
**Status**: ACTIVE COORDINATION

---

## Executive Summary

AIR-004 implementation is ~40% complete with critical foundation in place. The previous swarm successfully implemented:
- Core types: `StreamRecord`, `StreamConfig`, `SchemaField` in `/workspaces/neural-data-platform/core/src/types/`
- Stream registry wrapper in `/workspaces/neural-data-platform/config-client/src/stream/registry.rs`
- Architecture ADRs and test strategy documents

**CRITICAL BLOCKERS IDENTIFIED**:
1. **5 API tests FAILING** - routes not registered correctly (HIGH PRIORITY)
2. **Multi-stream coordinator NOT IMPLEMENTED** - core AIR-004 feature missing
3. **HttpPoller source NOT INTEGRATED** - code exists but not wired up
4. **WebhookHandler NOT IMPLEMENTED** - required for multi-source ingestion
5. **Production unwrap() usage** - 146+ instances causing crash risk

---

## Swarm Structure

### Hierarchical Coordination Model

```
    👑 SWARMLEAD (This Agent)
   /    |    |    \
  🔧   💻   🧪   📦
FIXER CODE TEST DEPLOY
AGENT AGENT AGENT AGENT
```

### Agent Assignments

#### Agent 1: Route Fixer (backend-dev)
**Priority**: CRITICAL - BLOCKING ALL OTHER WORK
**Task**: Fix 5 failing API route tests
**Duration**: 2-4 hours
**Dependencies**: None

**Failing Tests**:
- `test_aggregate_endpoint_mean` - 404 on `/api/v1/aggregate`
- `test_alerts_endpoint` - 404 on `/api/v1/alerts`
- `test_forecast_endpoint` - 404 on `/api/v1/forecast`
- `test_latest_readings_endpoint_with_data` - 404 on `/api/v1/readings/latest`
- `test_readings_time_range_query` - 404 on `/api/v1/readings`

**Root Cause Analysis**: Routes exist in test setup (`create_router()`) but may have incorrect routing logic or test server configuration.

**Success Criteria**:
- All 5 tests passing
- No impact on 54 passing tests
- Test report: `/workspaces/neural-data-platform/product/features/air-004/TEST_FIX_REPORT.md`

#### Agent 2: Coordinator Implementer (backend-dev + architect)
**Priority**: HIGH - Core AIR-004 feature
**Task**: Implement IngestionCoordinator + SourceManager
**Duration**: 8-12 hours
**Dependencies**: Agent 1 complete (test infrastructure stable)

**Deliverables**:
1. `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/ingestion_coordinator.rs`
2. `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`
3. `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/router.rs`
4. `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/mod.rs`

**Implementation Strategy** (from IMPLEMENTATION_PLAN.md Phase 4):
- IngestionCoordinator: Orchestrates all components (registry, sources, storage)
- SourceManager: Spawns and manages sources dynamically
- IngestionRouter: Validates and routes points to storage

**Success Criteria**:
- London TDD tests for all components
- Integration test demonstrating 2+ streams working
- Memory key: `swarm/coordinator/implementation-status`

#### Agent 3: Source Integration Specialist (backend-dev)
**Priority**: HIGH - Enables multi-source capability
**Task**: Integrate HttpPoller and implement WebhookHandler
**Duration**: 6-10 hours
**Dependencies**: Agent 2 in progress (can work in parallel on source implementations)

**Deliverables**:
1. Wire up existing `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`
2. Create `/workspaces/neural-data-platform/apps/air-quality-app/src/sources/webhook_handler.rs`
3. Create `/workspaces/neural-data-platform/apps/air-quality-app/src/sources/mqtt_wrapper.rs`
4. Update `/workspaces/neural-data-platform/apps/air-quality-app/src/sources/mod.rs`

**Implementation Strategy** (from IMPLEMENTATION_PLAN.md Phase 3):
- Follow MqttHandler pattern for consistency
- HttpPoller: tokio::interval polling with retry logic
- WebhookHandler: Axum server with authentication middleware
- All sources return `mpsc::Receiver<TimeSeriesPoint>`

**Success Criteria**:
- London TDD tests with mocked HTTP clients
- Integration tests with real endpoints (wiremock)
- Health check endpoints for all sources
- Memory key: `swarm/sources/integration-status`

#### Agent 4: Error Handling Auditor (reviewer + refactor-agent)
**Priority**: MEDIUM - Production quality improvement
**Task**: Replace production unwrap() with proper error handling
**Duration**: 4-6 hours (spread across implementation)
**Dependencies**: Can work in parallel

**Scope**:
- Review: `core/src/storage/parquet.rs`, `core/src/sources/`, `apps/air-quality-app/src/`
- Target: Production code only (tests can keep unwrap())
- Replace with: `?`, `map_err()`, or `expect()` with context

**Deliverables**:
- Modified files with error handling improvements
- Report: `/workspaces/neural-data-platform/product/features/air-004/ERROR_HANDLING_REPORT.md`

**Success Criteria**:
- Production unwrap() count reduced by >80%
- All tests still passing
- No new panics introduced
- Memory key: `swarm/quality/error-handling-status`

#### Agent 5: Integration Tester (tester)
**Priority**: MEDIUM - Validation and quality gates
**Task**: Create integration tests for multi-stream pipeline
**Duration**: 4-6 hours
**Dependencies**: Agent 2 and 3 near completion

**Deliverables**:
1. `/workspaces/neural-data-platform/apps/air-quality-app/tests/multi_stream_integration_test.rs`
2. Test scenarios: 2+ streams, dynamic addition, source failure recovery
3. Backward compatibility test with AIR-002 Parquet files

**Success Criteria** (from TEST_STRATEGY.md):
- 90% coverage for new coordinator code
- All baseline AIR-002/003 tests still passing
- Performance regression <10%
- Report: `/workspaces/neural-data-platform/product/features/air-004/INTEGRATION_TEST_REPORT.md`

---

## Critical Path Analysis

### Phase 1: Route Fix (BLOCKING - Days 1)
```
Agent 1 (Route Fixer) → MUST COMPLETE FIRST
├─ Fix test failures
├─ Verify no regressions
└─ Gate: All 59 tests passing
```

### Phase 2: Core Implementation (Days 2-4)
```
Agent 2 (Coordinator)     Agent 3 (Sources)        Agent 4 (Error Handling)
├─ IngestionCoordinator   ├─ HttpPoller wire-up    ├─ Audit unwrap() calls
├─ SourceManager          ├─ WebhookHandler        ├─ Replace in parquet.rs
├─ IngestionRouter        └─ MqttWrapper           └─ Replace in sources
└─ Integration            └─ Integration
    ↓                         ↓
    └─────── Converge ────────┘
```

### Phase 3: Integration & Testing (Days 5-6)
```
Agent 5 (Tester)
├─ Multi-stream integration tests
├─ Backward compatibility tests
└─ Performance validation
```

### Phase 4: Deployment Prep (Day 7)
```
All Agents
├─ Documentation updates
├─ Deployment config for Pi
└─ Final review and handoff
```

---

## Coordination Protocols

### Daily Sync Points
**Time**: Every 4 hours
**Method**: Memory store updates + status reports

**Memory Keys**:
- `swarm/coordinator/daily-status` - Overall progress
- `swarm/coordinator/blockers` - Any blocking issues
- `swarm/coordinator/decisions` - Architectural decisions
- `swarm/agent/{agent-name}/status` - Individual agent status

### Blocker Escalation
**Severity Levels**:
1. **P0 (CRITICAL)**: Blocks all work → Escalate immediately
2. **P1 (HIGH)**: Blocks specific agent → Escalate within 2 hours
3. **P2 (MEDIUM)**: Slows progress → Escalate within 4 hours

**Escalation Path**: Agent → SwarmLead → Manual review

### Quality Gates

#### Gate 1: Route Fix Complete (End of Day 1)
- ✅ All 59 tests passing
- ✅ No new warnings or errors
- ✅ Test report generated

#### Gate 2: Coordinator Implementation (End of Day 3)
- ✅ IngestionCoordinator working with 2+ streams
- ✅ London TDD tests passing (90% coverage)
- ✅ Integration test demonstrates multi-stream

#### Gate 3: Source Integration (End of Day 4)
- ✅ HttpPoller integrated and tested
- ✅ WebhookHandler implemented and tested
- ✅ Health checks operational

#### Gate 4: Production Ready (End of Day 6)
- ✅ All integration tests passing
- ✅ Backward compatibility verified
- ✅ Performance regression <10%
- ✅ Production unwrap() reduced >80%

---

## Risk Mitigation

### Risk 1: Test Failures Cascade
**Probability**: Medium
**Impact**: High (blocks all work)
**Mitigation**: Agent 1 works in isolation, runs tests after each change

### Risk 2: Coordinator Complexity
**Probability**: Medium
**Impact**: High (core feature delays)
**Mitigation**: Break into smaller components (Router, Manager, Coordinator), test each independently

### Risk 3: Integration Incompatibilities
**Probability**: Low
**Impact**: Medium
**Mitigation**: Frequent integration points, use existing patterns (MqttHandler)

### Risk 4: Performance Regression
**Probability**: Low
**Impact**: Medium
**Mitigation**: Benchmark at each phase gate, optimize before next phase

---

## Communication Plan

### Agent Check-ins
**Format**: JSON status updates to memory store
```json
{
  "agent": "route-fixer",
  "timestamp": "2025-12-15T10:00:00Z",
  "status": "in_progress",
  "progress": 60,
  "completed_tasks": ["Fixed aggregate endpoint", "Fixed alerts endpoint"],
  "blockers": [],
  "next_steps": ["Fix forecast endpoint", "Fix readings endpoints"]
}
```

### Swarm Reports
**Frequency**: Every 6 hours
**Format**: Markdown summary to `/workspaces/neural-data-platform/product/features/air-004/SWARM_STATUS.md`

### Final Deliverables
1. **Implementation Summary**: `/workspaces/neural-data-platform/product/features/air-004/IMPLEMENTATION_COMPLETE.md`
2. **Test Results**: All test reports consolidated
3. **Architecture Updates**: Updated ADRs if needed
4. **Deployment Guide**: Pi deployment instructions

---

## Success Metrics

### Code Quality
- ✅ All tests passing (59/59)
- ✅ Test coverage >90% for new code
- ✅ Production unwrap() <20 instances
- ✅ No new compiler warnings

### Functionality
- ✅ Multi-stream coordinator operational
- ✅ 3+ source types integrated (MQTT, HTTP, Webhook)
- ✅ Stream isolation verified (separate storage partitions)
- ✅ Dynamic stream addition working

### Performance
- ✅ Ingestion throughput: >1 msg/sec per stream
- ✅ Config read latency: <10ms
- ✅ API response time: <200ms p95
- ✅ Memory usage: <896MB on Pi

### Backward Compatibility
- ✅ Existing air-quality stream working
- ✅ AIR-002/003 Parquet files readable
- ✅ Existing API endpoints unchanged
- ✅ No breaking configuration changes

---

## Next Steps (Immediate)

### Hour 0-2: Route Fix Sprint
1. SwarmLead spawns Agent 1 (Route Fixer)
2. Agent 1 analyzes route configuration
3. Agent 1 fixes tests one by one
4. Agent 1 reports completion to memory store

### Hour 2-4: Coordinator Design
1. SwarmLead spawns Agent 2 (Coordinator Implementer)
2. Agent 2 creates component interfaces
3. Agent 2 writes London TDD tests
4. Agent 2 begins implementation

### Hour 4-6: Source Integration
1. SwarmLead spawns Agent 3 (Source Specialist)
2. Agent 3 wires up HttpPoller
3. Agent 3 begins WebhookHandler implementation
4. Agent 3 writes integration tests

### Parallel: Error Handling Audit
1. SwarmLead spawns Agent 4 (Error Handler)
2. Agent 4 audits unwrap() usage
3. Agent 4 replaces in critical paths
4. Agent 4 validates no new panics

---

## Memory Store Structure

```
swarm/
├── coordinator/
│   ├── status                    # Overall coordination status
│   ├── decisions                 # Architectural decisions
│   ├── blockers                  # Current blockers
│   └── tasks/
│       ├── route-fix-status
│       ├── coordinator-impl-status
│       ├── source-integration-status
│       └── error-handling-status
├── agent/
│   ├── route-fixer/
│   │   ├── progress
│   │   ├── completed-tasks
│   │   └── blockers
│   ├── coordinator-implementer/
│   │   ├── progress
│   │   └── design-decisions
│   ├── source-specialist/
│   │   └── integration-status
│   └── error-handler/
│       └── audit-results
└── reports/
    ├── daily-summary
    ├── test-results
    └── performance-metrics
```

---

## Coordination Agent Responsibilities

### SwarmLead (This Agent)
- ✅ Spawn and manage specialist agents
- ✅ Monitor progress through memory store
- ✅ Enforce quality gates
- ✅ Handle blocker escalation
- ✅ Generate status reports
- ✅ Coordinate handoffs between phases
- ✅ Validate deliverables
- ✅ Ensure backward compatibility

### Specialist Agents
- ✅ Execute assigned tasks
- ✅ Update memory store regularly
- ✅ Report blockers immediately
- ✅ Follow London TDD approach
- ✅ Write tests before implementation
- ✅ Document architectural decisions
- ✅ Validate against success criteria

---

**Status**: COORDINATION PLAN APPROVED
**Next Action**: Spawn Agent 1 (Route Fixer) to resolve blocking test failures
**Estimated Completion**: 6-7 days
**Confidence Level**: HIGH (foundation complete, clear path forward)

---

*Coordination Plan Version 1.0*
*Generated by SwarmLead Coordinator*
*Last Updated: 2025-12-15*
