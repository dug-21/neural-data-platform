# SPARC Plan: dp-017 Integration Test Harness

**Feature**: Integration Test Harness for Deployment Evolution
**Created**: 2026-02-01
**Coordinator**: ndp-scrum-master

---

## Executive Summary

dp-017 creates a reliable integration testing environment that mirrors production. This enables safe testing of `deploy.sh` commands before Pi deployment. The feature is approximately 70% complete with infrastructure alignment done; remaining work focuses on command verification and test harness creation.

---

## Phase Overview

| Phase | Status | Owner | Deliverable |
|-------|--------|-------|-------------|
| Specification | In Progress | ndp-architect | Requirements, acceptance criteria |
| Pseudocode | In Progress | ndp-architect | Test harness algorithm |
| Architecture | In Progress | ndp-architect | Component design, ADRs |
| Refinement | Pending | ndp-tester | TDD implementation |
| Completion | Pending | ndp-scrum-master | Verification, documentation |

---

## 1. Specification Phase

### Requirements Analysis

**Functional Requirements**:
1. All `deploy.sh` commands MUST work with `DEPLOY_ENV=integration`
2. Integration compose file MUST match production services
3. Test harness script MUST verify end-to-end data flow
4. Configuration sync MUST populate etcd correctly
5. Data dictionary sync MUST populate TimescaleDB correctly

**Non-Functional Requirements**:
1. Integration environment starts in < 2 minutes
2. Full test cycle completes in < 10 minutes
3. No manual intervention required for test execution
4. Clean teardown leaves no orphaned resources

### Commands to Verify

Based on `deploy/pi/deploy.sh` analysis:

| Command | Category | Integration Support | Status |
|---------|----------|---------------------|--------|
| `deploy` | Core | DEPLOY_ENV switching | Needs verification |
| `start` | Core | DEPLOY_ENV switching | Needs verification |
| `stop` | Core | DEPLOY_ENV switching | Needs verification |
| `status` | Core | DEPLOY_ENV switching | Needs verification |
| `build` | Core | DEPLOY_ENV switching | Needs verification |
| `logs` | Core | DEPLOY_ENV switching | Needs verification |
| `sync` | Config | Uses ETCD_CONTAINER | In Progress |
| `init-streams` | Config | Uses ETCD_CONTAINER | Pending |
| `list-streams` | Config | Uses ETCD_CONTAINER | Pending |
| `sync-dictionary` | Config | Uses dcx() helper | Pending |
| `sync-dimensions` | Config | Uses dcx() helper | Pending |
| `silver-migrate` | Silver | Uses dcx() helper | Pending |
| `silver-etl` | Silver | Uses profile flag | Pending |
| `update` | Update | DEPLOY_ENV switching | Out of scope |
| `refresh` | Update | DEPLOY_ENV switching | Out of scope |

### Acceptance Criteria

```gherkin
Feature: Integration Environment Commands

  Scenario: Full deployment cycle
    Given DEPLOY_ENV is set to "integration"
    When I run "./deploy.sh deploy"
    Then all services should start
    And etcd should be healthy
    And TimescaleDB should accept connections
    And mosquitto should accept pub/sub

  Scenario: Configuration sync
    Given the integration stack is running
    When I run "./deploy.sh sync"
    Then etcd should contain environment config
    And the config should match development environment

  Scenario: Stream initialization
    Given the integration stack is running
    When I run "./deploy.sh init-streams"
    Then etcd should contain stream configurations
    And stream count should match config/base/streams/

  Scenario: Data dictionary sync
    Given the integration stack is running
    When I run "./deploy.sh sync-dictionary"
    Then data_dictionary.streams should be populated
    And data_dictionary.silver_tables should be populated
    And sync_status should show success

  Scenario: Clean shutdown
    Given the integration stack is running
    When I run "./deploy.sh stop"
    Then all containers should stop
    And no orphan processes remain
```

---

## 2. Pseudocode Phase

### Test Harness Algorithm

```
PROGRAM integration_test_harness

CONSTANTS:
    TIMEOUT_STARTUP = 120 seconds
    TIMEOUT_COMMAND = 60 seconds
    COMPOSE_FILE = "docker-compose.integration.yml"

FUNCTION main():
    results = TestResults()

    TRY:
        # Phase 1: Environment Setup
        cleanup_previous_run()
        verify_prerequisites()

        # Phase 2: Core Commands
        results.add(test_deploy())
        results.add(test_status())

        # Phase 3: Configuration Commands
        results.add(test_sync())
        results.add(test_init_streams())
        results.add(test_sync_dictionary())

        # Phase 4: Data Flow Verification
        results.add(test_mqtt_to_bronze())
        results.add(test_bronze_to_silver())

        # Phase 5: Cleanup
        results.add(test_stop())

    FINALLY:
        generate_report(results)
        cleanup_integration_env()

    RETURN results.all_passed()

FUNCTION test_deploy():
    SET DEPLOY_ENV = "integration"
    result = run_command("./deploy.sh deploy", timeout=TIMEOUT_STARTUP)

    IF result.exit_code != 0:
        RETURN TestResult(FAIL, "Deploy failed", result.stderr)

    # Verify all services started
    services = ["etcd", "mosquitto", "timescaledb", "air-quality-app"]
    FOR service IN services:
        IF NOT container_healthy(service):
            RETURN TestResult(FAIL, f"{service} not healthy")

    RETURN TestResult(PASS, "All services started")

FUNCTION test_sync():
    result = run_command("./deploy.sh sync", timeout=TIMEOUT_COMMAND)

    IF result.exit_code != 0:
        RETURN TestResult(FAIL, "Sync failed", result.stderr)

    # Verify etcd has config
    etcd_keys = get_etcd_keys("/air-quality/")
    IF len(etcd_keys) == 0:
        RETURN TestResult(FAIL, "No config in etcd")

    RETURN TestResult(PASS, f"Synced {len(etcd_keys)} keys")

FUNCTION test_init_streams():
    result = run_command("./deploy.sh init-streams", timeout=TIMEOUT_COMMAND)

    IF result.exit_code != 0:
        RETURN TestResult(FAIL, "init-streams failed", result.stderr)

    # Verify streams in etcd
    stream_keys = get_etcd_keys("/air-quality/streams/")
    expected_streams = count_stream_configs()

    IF len(stream_keys) < expected_streams:
        RETURN TestResult(FAIL, f"Expected {expected_streams} streams, found {len(stream_keys)}")

    RETURN TestResult(PASS, f"Initialized {len(stream_keys)} streams")

FUNCTION test_sync_dictionary():
    result = run_command("./deploy.sh sync-dictionary", timeout=TIMEOUT_COMMAND)

    IF result.exit_code != 0:
        RETURN TestResult(FAIL, "sync-dictionary failed", result.stderr)

    # Verify data dictionary populated
    sync_status = query_db("SELECT status FROM data_dictionary.sync_status ORDER BY id DESC LIMIT 1")

    IF sync_status != "success":
        RETURN TestResult(FAIL, f"Sync status: {sync_status}")

    RETURN TestResult(PASS, "Data dictionary synced")

FUNCTION test_mqtt_to_bronze():
    # Publish test message
    test_payload = {"temperature": 25.0, "humidity": 50.0}
    mqtt_publish("homeassistant/sensor/test/state", test_payload)

    WAIT 5 seconds

    # Verify Bronze file created
    bronze_files = list_bronze_files("air-quality")

    IF len(bronze_files) == 0:
        RETURN TestResult(FAIL, "No Bronze files created")

    RETURN TestResult(PASS, "MQTT -> Bronze working")

FUNCTION test_stop():
    result = run_command("./deploy.sh stop", timeout=TIMEOUT_COMMAND)

    IF result.exit_code != 0:
        RETURN TestResult(FAIL, "Stop failed", result.stderr)

    # Verify all containers stopped
    running = get_running_containers("integration")

    IF len(running) > 0:
        RETURN TestResult(FAIL, f"Containers still running: {running}")

    RETURN TestResult(PASS, "Clean shutdown")

END PROGRAM
```

---

## 3. Architecture Phase

### Component Diagram

```
+------------------------------------------------------------------+
|                    Integration Test Environment                    |
+------------------------------------------------------------------+
|                                                                    |
|  +----------------+     +------------------+     +---------------+ |
|  | Test Harness   |---->| deploy.sh        |---->| Docker        | |
|  | (bash script)  |     | DEPLOY_ENV=integ |     | Compose       | |
|  +----------------+     +------------------+     +---------------+ |
|         |                                              |           |
|         v                                              v           |
|  +----------------+                         +------------------+   |
|  | Test Assertions|                         | integration-*    |   |
|  | - etcd keys    |                         | containers       |   |
|  | - DB queries   |                         +------------------+   |
|  | - MQTT pub/sub |                                |               |
|  +----------------+                                v               |
|                                            +------------------+    |
|                                            | Shared Network   |    |
|                                            | ndp-integration  |    |
|                                            +------------------+    |
+------------------------------------------------------------------+
```

### Container Name Mapping

| Service | Production Container | Integration Container |
|---------|---------------------|----------------------|
| etcd | `etcd` | `integration-etcd` |
| mosquitto | `mosquitto` | `integration-mosquitto` |
| timescaledb | `timescaledb` | `integration-timescaledb` |
| air-quality-app | `air-quality-app` | `integration-air-quality-app` |
| grafana | `grafana` | `integration-grafana` |
| ndp-mcp-server | `ndp-mcp-server` | `integration-ndp-mcp-server` |

### Key Design Decisions

**ADR-pending-001: Container Naming Convention**
- Decision: Integration containers use `integration-` prefix
- Rationale: Prevents collision with production containers if both run
- Status: Implemented in docker-compose.integration.yml

**ADR-pending-002: ETCD_CONTAINER Variable**
- Decision: deploy.sh uses ETCD_CONTAINER to abstract container name
- Rationale: Scripts work regardless of environment
- Status: Implemented (line 64-65 of deploy.sh)

**ADR-pending-003: dcx() Helper Pattern**
- Decision: Use `dcx()` for all docker compose exec calls
- Rationale: Consistent service name handling across environments
- Status: Implemented (line 79-81 of deploy.sh)

---

## 4. Implementation Sequence

### Critical Path (Sequential)

```
1. [DONE] Infrastructure alignment
   - Update docker-compose.integration.yml
   - Remove silver-etl-daemon
   - Add missing services

2. [IN PROGRESS] Configuration command verification
   - Test sync command
   - Test init-streams command
   - Test sync-dictionary command

3. [PENDING] Test harness creation
   - Create scripts/integration-test.sh
   - Implement test functions
   - Add CI integration hooks

4. [PENDING] Data flow verification
   - MQTT -> Bronze test
   - Bronze -> Silver test (if silver-etl enabled)
```

### Parallelizable Tasks

```
+-- sync command testing
|
+-- init-streams command testing     (can run in parallel after stack up)
|
+-- sync-dictionary command testing
```

```
+-- Test harness script structure
|
+-- Test assertion library           (can run in parallel)
|
+-- CI integration documentation
```

---

## 5. Risk Assessment

### High Risk

| Risk | Impact | Mitigation |
|------|--------|------------|
| Container name collision | Tests fail unpredictably | Use dedicated network, unique prefixes |
| etcd data persistence | Stale config between runs | Add cleanup step, use ephemeral volumes |
| Port conflicts | Stack fails to start | Use non-standard ports in integration |

### Medium Risk

| Risk | Impact | Mitigation |
|------|--------|------------|
| init-scripts path issues | TimescaleDB schema missing | Verified in Task 6 (complete) |
| YAML parsing differences | Sync commands fail | Test with both yq variants |
| Slow CI execution | Long feedback loops | Cache Docker layers, parallel tests |

### Unknowns Requiring Research

| Unknown | Owner | Research Needed |
|---------|-------|-----------------|
| Silver ETL in integration | ndp-tester | Does profile flag work? |
| Data volume cleanup | ndp-rust-dev | Best practice for ephemeral data |
| GitHub Actions integration | ndp-scrum-master | How to run integration tests in CI |

---

## 6. Definition of Done

### Required for Completion

- [ ] All deploy.sh commands work with `DEPLOY_ENV=integration`
  - [ ] deploy
  - [ ] start
  - [ ] stop
  - [ ] status
  - [ ] sync
  - [ ] init-streams
  - [ ] sync-dictionary

- [ ] Test harness script exists at `scripts/integration-test.sh`
  - [ ] Runs all verification steps
  - [ ] Reports pass/fail status
  - [ ] Cleans up after itself

- [ ] Documentation updated
  - [ ] README mentions integration testing
  - [ ] deploy.sh --help is accurate
  - [ ] SPARC Completion document exists

### Optional Enhancements (Future Work)

- [ ] GitHub Actions workflow for integration tests
- [ ] Data flow verification (MQTT -> Bronze -> Silver)
- [ ] Performance benchmarks in integration mode
- [ ] Root compose file audit (deferred from dp-017)

---

## 7. Coordination Notes

### Agent Assignments

| Agent | Responsibility |
|-------|----------------|
| ndp-architect | Specification, Architecture, ADRs |
| ndp-rust-dev | Script implementation if needed |
| ndp-tester | Refinement phase, test implementation |
| ndp-scrum-master | Status tracking, phase transitions |

### Swarm Communication

Progress updates should be written to:
- `product/features/dp-017/STATUS.md` (status changes)
- `product/features/dp-017/reports/` (swarm coordination reports)

### Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-016 Config Architecture Review | Blocking | Required for safe deployment testing |
| docker-compose.integration.yml | Complete | Aligned with production |
| deploy/pi/deploy.sh | Stable | Reference implementation |

---

*Last updated: 2026-02-01 by ndp-scrum-master*
