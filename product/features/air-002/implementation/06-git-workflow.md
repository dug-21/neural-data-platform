# AIR-002: Git Workflow and Branch Management

**Feature:** MQTT to Parquet Ingestion Pipeline
**Current Branch:** feature/air-001-implementation
**Target Branch:** feature/air-002-ingestion-pipeline
**Base Branch:** main
**Created:** 2025-12-14

---

## Current State Analysis

### Current Branch Status
- **Branch:** `feature/air-001-implementation`
- **Last Commit:** `c17bff2` - "feat(air-001): implement AirGradient air quality monitoring platform"
- **Modified Files:**
  - `.claude-flow/metrics/performance.json`
  - `.claude-flow/metrics/system-metrics.json`
  - `.claude-flow/metrics/task-metrics.json`
- **Untracked Files:**
  - `docs/architecture/` - Config store client documentation (5 files)
  - `product/features/air-001/current-state/` - AIR-001 analysis
  - `product/features/air-002/` - AIR-002 specifications and planning

### Branch Decision
We are currently on `feature/air-001-implementation`, but AIR-002 is a distinct feature. We should create a new feature branch for AIR-002 to maintain clean separation of concerns.

**Recommendation:** Create `feature/air-002-ingestion-pipeline` branch from current state.

---

## Git Workflow for AIR-002

### Branch Creation
```bash
# Ensure we're on the correct starting point
git checkout feature/air-001-implementation

# Create new feature branch for AIR-002
git checkout -b feature/air-002-ingestion-pipeline

# Verify branch creation
git branch --show-current
```

### Branch Naming Convention
- **Pattern:** `feature/air-{number}-{short-description}`
- **AIR-002 Branch:** `feature/air-002-ingestion-pipeline`
- **Rationale:**
  - Clear feature identification
  - Easy to track in GitHub
  - Follows existing pattern (air-001)

---

## Commit Strategy

### Commit Grouping
Based on the implementation roadmap, commits will be organized by functional milestone:

#### Commit 1: Configuration Foundation (T1)
**Scope:** Configuration management
**Files:**
- `apps/air-quality-app/config.yaml` (CREATE)
- `apps/air-quality-app/src/config.rs` (MODIFY)

**Message:**
```
feat(air-002): add YAML configuration for MQTT and storage

- Create config.yaml with MQTT broker and storage settings
- Update config.rs with MqttConfigYaml and StorageConfigYaml structs
- Add environment variable override support (MQTT_BROKER_URL, STORAGE_PATH)
- Implement config validation and error handling
- Add conversion from YAML config to platform-core types

Part of AIR-002 Task 1 (Configuration Management)
```

#### Commit 2: Ingestion Module (T2)
**Scope:** MQTT handler and message processing
**Files:**
- `apps/air-quality-app/src/ingestion/mod.rs` (CREATE)
- `apps/air-quality-app/src/ingestion/mqtt_handler.rs` (CREATE)
- `apps/air-quality-app/src/lib.rs` (MODIFY - add ingestion module)

**Message:**
```
feat(air-002): implement MQTT ingestion handler

- Create ingestion module with MqttHandler
- Connect to MQTT broker and subscribe to airgradient/readings/+ topic
- Parse messages using air_quality::parser
- Validate readings using air_quality::validation
- Convert to TimeSeriesPoints using air_quality::adapter
- Send processed points to storage pipeline channel
- Implement auto-reconnection with exponential backoff

Part of AIR-002 Task 2 (MQTT Ingestion Module)
```

#### Commit 3: Storage Pipeline (T3)
**Scope:** Parquet storage writer with batching
**Files:**
- `apps/air-quality-app/src/pipeline/mod.rs` (CREATE)
- `apps/air-quality-app/src/pipeline/storage_writer.rs` (CREATE)
- `apps/air-quality-app/src/lib.rs` (MODIFY - add pipeline module)

**Message:**
```
feat(air-002): implement storage pipeline with batching

- Create pipeline module with StorageWriter
- Implement batched writes (100 points or 5 second timeout)
- Use ParquetStore with WAL for durability
- Add graceful error handling and retry logic
- Implement backpressure handling

Part of AIR-002 Task 3 (Storage Pipeline)
```

#### Commit 4: Main Integration (T4)
**Scope:** Wire all components together in main.rs
**Files:**
- `apps/air-quality-app/src/main.rs` (MODIFY)
- `apps/air-quality-app/Cargo.toml` (MODIFY - dependencies)

**Message:**
```
feat(air-002): integrate MQTT ingestion pipeline in main

- Replace mock services with real MQTT and storage implementations
- Initialize MqttHandler and StorageWriter on startup
- Create mpsc channel for MQTT -> storage data flow
- Spawn background tasks for ingestion and storage
- Implement WAL replay on startup
- Add graceful shutdown with WAL commit
- Remove mock implementations

Part of AIR-002 Task 4 (Main Integration)
```

#### Commit 5: Health Endpoint (T5)
**Scope:** Real health status reporting
**Files:**
- `apps/air-quality-app/src/api/handlers/health.rs` (MODIFY)

**Message:**
```
feat(air-002): update health endpoint with real status checks

- Query actual MQTT connection status
- Check storage health via ParquetStore
- Return overall status: healthy/degraded/unhealthy
- Include detailed component status and messages
- Add timestamp to health response

Part of AIR-002 Task 5 (Health Endpoint Integration)
```

#### Commit 6: Integration Tests (T6)
**Scope:** End-to-end integration tests
**Files:**
- `apps/air-quality-app/tests/integration_test.rs` (CREATE)
- `apps/air-quality-app/Cargo.toml` (MODIFY - test dependencies)

**Message:**
```
test(air-002): add integration tests for ingestion pipeline

- Test MQTT to Parquet data flow
- Test data persistence across restarts
- Test health endpoint accuracy
- Test invalid message handling
- Test WAL recovery after crash
- Use testcontainers for MQTT broker isolation

Part of AIR-002 Task 6 (Integration Tests)
```

#### Commit 7: Documentation
**Scope:** Update documentation and examples
**Files:**
- `apps/air-quality-app/README.md` (CREATE/MODIFY)
- `apps/air-quality-app/config.yaml.example` (CREATE)
- `product/features/air-002/IMPLEMENTATION_REPORT.md` (CREATE)

**Message:**
```
docs(air-002): add documentation for ingestion pipeline

- Document MQTT configuration options
- Add config.yaml.example with all settings
- Create implementation report with verification results
- Update README with setup and usage instructions
- Add troubleshooting section

Part of AIR-002 Task 6 (Documentation)
```

---

## Files to be Created/Modified

### New Files (15 total)
```
apps/air-quality-app/
├── config.yaml                              (T1)
├── config.yaml.example                      (T6 - docs)
├── README.md                                (T6 - docs)
├── src/
│   ├── ingestion/
│   │   ├── mod.rs                          (T2)
│   │   └── mqtt_handler.rs                 (T2)
│   └── pipeline/
│       ├── mod.rs                          (T3)
│       └── storage_writer.rs               (T3)
└── tests/
    └── integration_test.rs                  (T6)

product/features/air-002/
└── implementation/
    ├── 06-git-workflow.md                   (this file)
    └── IMPLEMENTATION_REPORT.md             (T6 - docs)
```

### Modified Files (5 total)
```
apps/air-quality-app/
├── Cargo.toml                               (T4, T6 - dependencies)
├── src/
│   ├── main.rs                             (T4)
│   ├── lib.rs                              (T2, T3 - module declarations)
│   ├── config.rs                           (T1)
│   └── api/handlers/
│       └── health.rs                       (T5)
```

### Claude Flow Metrics (automatic updates)
```
.claude-flow/metrics/
├── performance.json
├── system-metrics.json
└── task-metrics.json
```

---

## Commit Message Format

### Standard Format
```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types
- `feat`: New feature
- `test`: Adding/updating tests
- `docs`: Documentation only
- `refactor`: Code change without feature change
- `chore`: Maintenance tasks

### Scope
- `air-002`: All commits related to AIR-002 feature
- Can be more specific: `air-002-config`, `air-002-mqtt`, etc.

### Examples

**Good:**
```
feat(air-002): implement MQTT ingestion handler

- Connect to broker using rumqttc
- Subscribe to airgradient/readings/+ topic
- Parse and validate incoming messages
- Convert to TimeSeriesPoints

Part of AIR-002 Task 2
```

**Bad:**
```
Update files
```

---

## Pre-Commit Checklist

Before each commit, verify:
- [ ] Code compiles: `cargo build`
- [ ] Tests pass: `cargo test`
- [ ] Linting passes: `cargo clippy -- -D warnings`
- [ ] Formatting applied: `cargo fmt`
- [ ] No debug prints or temporary code
- [ ] Commit message follows format
- [ ] Only related changes included

---

## Branch Protection Strategy

### Before Merge to Main
- [ ] All commits made and pushed
- [ ] All integration tests passing
- [ ] Code reviewed by at least one other developer
- [ ] No merge conflicts with main
- [ ] CI/CD pipeline green
- [ ] Performance benchmarks met
- [ ] Documentation updated

### Merge Strategy
**Recommendation:** Squash and merge

**Rationale:**
- Keeps main branch history clean
- Single commit per feature
- Easier to revert if needed
- All commits preserved in PR

**Final commit message:**
```
feat(air-002): implement MQTT to Parquet ingestion pipeline

Completes AIR-002 implementation:
- YAML configuration with environment overrides
- MQTT handler with auto-reconnection
- Batched Parquet storage with WAL
- Real health status reporting
- Full integration test suite

This unblocks end-to-end testing of the air quality platform
by enabling real sensor data flow from MQTT to storage.

Closes #AIR-002
```

---

## Git Commands Reference

### Daily Workflow
```bash
# Check status
git status

# Stage specific files
git add apps/air-quality-app/src/ingestion/mqtt_handler.rs

# Commit with message
git commit -m "feat(air-002): implement MQTT handler"

# Push to remote
git push origin feature/air-002-ingestion-pipeline

# Pull latest changes
git pull origin feature/air-002-ingestion-pipeline
```

### Sync with Main
```bash
# Fetch latest main
git fetch origin main

# Rebase on main (if needed)
git rebase origin/main

# Resolve conflicts if any
# ... edit conflicted files ...
git add <resolved-files>
git rebase --continue

# Force push (only after rebase)
git push --force-with-lease origin feature/air-002-ingestion-pipeline
```

### Undo Operations
```bash
# Undo uncommitted changes
git checkout -- apps/air-quality-app/src/main.rs

# Undo last commit (keep changes)
git reset HEAD~1

# Undo last commit (discard changes)
git reset --hard HEAD~1

# Amend last commit
git commit --amend
```

---

## Pull Request Guidelines

### PR Title
```
feat(air-002): Implement MQTT to Parquet ingestion pipeline
```

### PR Description Template
```markdown
## Summary
Implements the complete ingestion pipeline for AIR-002, enabling real sensor data flow from MQTT broker to Parquet storage.

## Changes
- Configuration management with YAML and env overrides (T1)
- MQTT ingestion handler with auto-reconnection (T2)
- Batched storage writer with WAL (T3)
- Main application integration (T4)
- Real health status reporting (T5)
- Comprehensive integration test suite (T6)

## Testing
- ✅ All unit tests passing
- ✅ All integration tests passing
- ✅ Manual verification with real MQTT broker
- ✅ Performance benchmarks met (1s latency p95)
- ✅ 24-hour soak test completed

## Verification Steps
1. Start Mosquitto broker: `docker run -p 1883:1883 eclipse-mosquitto`
2. Start application: `cargo run --bin air-quality-app`
3. Publish test message: `mosquitto_pub -t "airgradient/readings/test" -m '{"serialno":"test","pm02":12.5}'`
4. Query API: `curl http://localhost:8080/api/v1/readings/latest?location_id=test`
5. Verify data returned

## Breaking Changes
None - this is additive functionality

## Related Issues
Closes #AIR-002
Blocks AIR-003, AIR-004, AIR-005

## Checklist
- [x] Code compiles without warnings
- [x] All tests pass
- [x] Documentation updated
- [x] Config example provided
- [x] Manual verification completed
- [x] Performance benchmarks met
```

---

## Risk Mitigation

### Potential Issues During Development

**Issue:** Conflicts with main branch
- **Prevention:** Regularly sync with main (`git fetch origin main`)
- **Resolution:** Rebase early and often

**Issue:** Accidentally committing sensitive data
- **Prevention:** Use `.gitignore` for config.yaml (use config.yaml.example instead)
- **Resolution:** `git filter-branch` or BFG Repo-Cleaner

**Issue:** Lost work due to local changes
- **Prevention:** Commit frequently, push to remote daily
- **Resolution:** `git reflog` to recover lost commits

**Issue:** Merge conflicts in Cargo.lock
- **Prevention:** Coordinate dependency changes
- **Resolution:** Regenerate with `cargo generate-lockfile`

---

## Timeline and Milestones

### Week 1 (Days 1-2)
- **Day 1:** T1 (Config) + T2 (MQTT Handler)
  - Commit 1: Configuration
  - Commit 2: MQTT Handler
  - Push to remote

- **Day 2:** T3 (Storage Pipeline) + T4 (Main Integration)
  - Commit 3: Storage Pipeline
  - Commit 4: Main Integration
  - Push to remote
  - Manual verification checkpoint

### Week 1 (Days 3-4)
- **Day 3:** T5 (Health) + T6 (Integration Tests - Part 1)
  - Commit 5: Health Endpoint
  - Begin integration tests

- **Day 4:** T6 (Integration Tests - Part 2) + Documentation
  - Commit 6: Integration Tests
  - Commit 7: Documentation
  - Final push
  - Create pull request

---

## Post-Implementation

### After PR Merge
1. Delete feature branch (local and remote)
   ```bash
   git branch -d feature/air-002-ingestion-pipeline
   git push origin --delete feature/air-002-ingestion-pipeline
   ```

2. Tag release on main
   ```bash
   git checkout main
   git pull origin main
   git tag -a v0.2.0-air-002 -m "AIR-002: MQTT to Parquet ingestion pipeline"
   git push origin v0.2.0-air-002
   ```

3. Update project documentation
   - Update main README with new features
   - Update architecture diagrams
   - Add to CHANGELOG.md

---

## References

- **AIR-002 Specification:** `/workspaces/neural-data-platform/product/features/air-002/specs/01-specification.md`
- **Implementation Roadmap:** `/workspaces/neural-data-platform/product/features/air-002/implementation/01-roadmap.md`
- **Parent Feature (AIR-001):** `/workspaces/neural-data-platform/product/features/air-001/`
- **Repository:** `https://github.com/dug-21/neural-data-platform.git`

---

**Document Status:** DRAFT
**Created:** 2025-12-14
**Last Updated:** 2025-12-14
**Approved By:** [Pending Review]
