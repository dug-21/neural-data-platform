# AIR-002: Quick Start Guide

**For:** Implementation team starting AIR-002 work
**Status:** Ready for implementation
**Estimated Time:** 3-4 days (22-30 hours)

---

## Before You Start

### Prerequisites
- [ ] Read `/product/features/air-002/specs/01-specification.md`
- [ ] Review `/product/features/air-002/implementation/01-roadmap.md`
- [ ] Understand git workflow: `/product/features/air-002/implementation/06-git-workflow.md`

### Setup
```bash
# Ensure you're in the project root
cd /workspaces/neural-data-platform

# Check current branch
git branch --show-current
# Should be: feature/air-001-implementation

# Pull latest changes
git pull origin feature/air-001-implementation
```

---

## Step 1: Create Feature Branch

```bash
# Create and switch to AIR-002 branch
git checkout -b feature/air-002-ingestion-pipeline

# Verify
git branch --show-current
# Expected: feature/air-002-ingestion-pipeline

# Push to remote (set upstream)
git push -u origin feature/air-002-ingestion-pipeline
```

---

## Step 2: Implementation Tasks (In Order)

### Task 1: Configuration (1-2 hours)
**Files:** `apps/air-quality-app/config.yaml`, `apps/air-quality-app/src/config.rs`

**What to do:**
1. Create `config.yaml` with MQTT and storage settings
2. Update `config.rs` with `MqttConfigYaml` and `StorageConfigYaml` structs
3. Add environment variable override support
4. Test: `MQTT_BROKER_URL=test cargo run --bin air-quality-app`

**When done:**
```bash
git add apps/air-quality-app/config.yaml apps/air-quality-app/src/config.rs
git commit -m "feat(air-002): add YAML configuration for MQTT and storage

- Create config.yaml with MQTT broker and storage settings
- Update config.rs with MqttConfigYaml and StorageConfigYaml structs
- Add environment variable override support
- Implement config validation and conversion to platform-core types

Part of AIR-002 Task 1"
git push
```

---

### Task 2: MQTT Handler (6-8 hours)
**Files:** `apps/air-quality-app/src/ingestion/mod.rs`, `apps/air-quality-app/src/ingestion/mqtt_handler.rs`

**What to do:**
1. Create `ingestion/` module
2. Implement `MqttHandler` struct
3. Connect to MQTT broker
4. Subscribe to `airgradient/readings/+`
5. Parse messages using `air_quality::parser`
6. Validate using `air_quality::validation`
7. Convert to `TimeSeriesPoint` using `air_quality::adapter`
8. Test: Publish MQTT message, verify parsing in logs

**When done:**
```bash
git add apps/air-quality-app/src/ingestion/ apps/air-quality-app/src/lib.rs
git commit -m "feat(air-002): implement MQTT ingestion handler

- Create ingestion module with MqttHandler
- Connect to MQTT broker and subscribe to topic
- Parse, validate, and convert messages to TimeSeriesPoints
- Implement auto-reconnection with exponential backoff

Part of AIR-002 Task 2"
git push
```

---

### Task 3: Storage Pipeline (5-6 hours)
**Files:** `apps/air-quality-app/src/pipeline/mod.rs`, `apps/air-quality-app/src/pipeline/storage_writer.rs`

**What to do:**
1. Create `pipeline/` module
2. Implement `StorageWriter` struct
3. Batch writes (100 points or 5 seconds)
4. Use `ParquetStore` with WAL
5. Handle errors with retry logic
6. Test: Verify Parquet files created in `/data/parquet`

**When done:**
```bash
git add apps/air-quality-app/src/pipeline/ apps/air-quality-app/src/lib.rs
git commit -m "feat(air-002): implement storage pipeline with batching

- Create pipeline module with StorageWriter
- Implement batched writes with timeout
- Use ParquetStore with WAL for durability
- Add error handling and retry logic

Part of AIR-002 Task 3"
git push
```

---

### Task 4: Main Integration (4-5 hours)
**Files:** `apps/air-quality-app/src/main.rs`, `apps/air-quality-app/Cargo.toml`

**What to do:**
1. Remove mock implementations
2. Initialize `MqttHandler` and `StorageWriter`
3. Create mpsc channel for data flow
4. Spawn background tasks
5. Implement WAL replay on startup
6. Add graceful shutdown
7. Test: End-to-end flow (MQTT → Parquet → API query)

**When done:**
```bash
git add apps/air-quality-app/src/main.rs apps/air-quality-app/Cargo.toml
git commit -m "feat(air-002): integrate MQTT ingestion pipeline in main

- Replace mock services with real implementations
- Initialize MQTT and storage components
- Create data pipeline with mpsc channel
- Add WAL replay and graceful shutdown

Part of AIR-002 Task 4"
git push
```

---

### Task 5: Health Endpoint (2-3 hours)
**Files:** `apps/air-quality-app/src/api/handlers/health.rs`

**What to do:**
1. Query actual MQTT connection status
2. Check storage health
3. Return overall status (healthy/degraded/unhealthy)
4. Test: `curl http://localhost:8080/health`

**When done:**
```bash
git add apps/air-quality-app/src/api/handlers/health.rs
git commit -m "feat(air-002): update health endpoint with real status checks

- Query actual MQTT connection status
- Check storage health via ParquetStore
- Return overall status with component details

Part of AIR-002 Task 5"
git push
```

---

### Task 6: Integration Tests (4-5 hours)
**Files:** `apps/air-quality-app/tests/integration_test.rs`, `apps/air-quality-app/Cargo.toml`

**What to do:**
1. Create integration test suite
2. Test MQTT → Parquet flow
3. Test data persistence across restarts
4. Test health endpoint accuracy
5. Test invalid message handling
6. Test WAL recovery
7. Test: `cargo test --test integration_test`

**When done:**
```bash
git add apps/air-quality-app/tests/integration_test.rs apps/air-quality-app/Cargo.toml
git commit -m "test(air-002): add integration tests for ingestion pipeline

- Test MQTT to Parquet data flow
- Test data persistence across restarts
- Test health endpoint accuracy
- Test error handling and WAL recovery

Part of AIR-002 Task 6"
git push
```

---

### Task 7: Documentation (1-2 hours)
**Files:** `apps/air-quality-app/README.md`, `apps/air-quality-app/config.yaml.example`, `product/features/air-002/IMPLEMENTATION_REPORT.md`

**What to do:**
1. Create README with setup instructions
2. Create `config.yaml.example`
3. Write implementation report
4. Document troubleshooting steps

**When done:**
```bash
git add apps/air-quality-app/README.md apps/air-quality-app/config.yaml.example product/features/air-002/IMPLEMENTATION_REPORT.md
git commit -m "docs(air-002): add documentation for ingestion pipeline

- Create README with setup and usage instructions
- Add config.yaml.example with all options
- Write implementation report with verification results

Part of AIR-002 Task 6 (Documentation)"
git push
```

---

## Step 3: Create Pull Request

### After All Commits Are Pushed

```bash
# Verify all tests pass
cargo test

# Verify formatting
cargo fmt --check

# Verify linting
cargo clippy -- -D warnings

# Check git status
git status
# Should show: "Your branch is up to date with 'origin/feature/air-002-ingestion-pipeline'"
```

### Create PR Using GitHub CLI
```bash
gh pr create \
  --title "feat(air-002): Implement MQTT to Parquet ingestion pipeline" \
  --body "$(cat <<'EOF'
## Summary
Implements the complete ingestion pipeline for AIR-002, enabling real sensor data flow from MQTT broker to Parquet storage.

## Changes
- Configuration management with YAML and env overrides (T1)
- MQTT ingestion handler with auto-reconnection (T2)
- Batched storage writer with WAL (T3)
- Main application integration (T4)
- Real health status reporting (T5)
- Comprehensive integration test suite (T6)
- Complete documentation (T7)

## Testing
- All unit tests passing
- All integration tests passing
- Manual verification with real MQTT broker completed
- Performance benchmarks met (1s latency p95)

## Verification Steps
1. Start Mosquitto broker: docker run -p 1883:1883 eclipse-mosquitto
2. Start application: cargo run --bin air-quality-app
3. Publish test message: mosquitto_pub -t "airgradient/readings/test" -m '{"serialno":"test","pm02":12.5}'
4. Query API: curl http://localhost:8080/api/v1/readings/latest?location_id=test
5. Verify data returned

## Breaking Changes
None - this is additive functionality

## Related Issues
Closes AIR-002
EOF
)" \
  --base main
```

**Or create PR via GitHub web interface:**
1. Go to https://github.com/dug-21/neural-data-platform/pulls
2. Click "New pull request"
3. Select base: `main`, compare: `feature/air-002-ingestion-pipeline`
4. Fill in title and description (see template above)
5. Click "Create pull request"

---

## Verification Checkpoints

### After Task 2 (MQTT)
```bash
# Start app
cargo run --bin air-quality-app

# Expected logs:
# INFO air_quality_app: MQTT handler started
# INFO rumqttc: Connection established

# Test message
mosquitto_pub -t "airgradient/readings/test" -m '{"serialno":"test","pm02":12.5,"rco2":450}'

# Expected logs:
# INFO air_quality_app::ingestion: Received 2 time series points
```

### After Task 4 (Integration)
```bash
# Publish message
mosquitto_pub -t "airgradient/readings/sensor1" -m '{"serialno":"sensor1","pm02":12.5}'

# Wait 5 seconds

# Check Parquet files
ls -lh /data/parquet/data/sensor1/year=*/month=*/day=*/

# Query API
curl http://localhost:8080/api/v1/readings/latest?location_id=sensor1
# Should return actual data, not mock
```

### After Task 5 (Health)
```bash
# Check health
curl http://localhost:8080/health | jq

# Expected:
# {
#   "status": "healthy",
#   "components": {
#     "mqtt": {"healthy": true},
#     "storage": {"healthy": true}
#   }
# }
```

---

## Common Commands

### Development
```bash
# Build
cargo build

# Run
cargo run --bin air-quality-app

# Run with custom config
MQTT_BROKER_URL=localhost cargo run --bin air-quality-app

# Test
cargo test

# Test specific test
cargo test --test integration_test

# Format
cargo fmt

# Lint
cargo clippy

# Check (faster than build)
cargo check
```

### Git
```bash
# Status
git status

# Add files
git add <files>

# Commit
git commit -m "message"

# Push
git push

# Pull
git pull

# View commits
git log --oneline -10

# View diff
git diff
```

### MQTT Testing
```bash
# Start Mosquitto (Docker)
docker run -d -p 1883:1883 --name mosquitto eclipse-mosquitto

# Publish message
mosquitto_pub -t "airgradient/readings/test123" -m '{"serialno":"test123","pm02":12.5,"rco2":450}'

# Subscribe to see messages
mosquitto_sub -t "airgradient/readings/+"

# Stop Mosquitto
docker stop mosquitto
docker rm mosquitto
```

---

## Troubleshooting

### Issue: MQTT connection fails
```bash
# Check if Mosquitto is running
docker ps | grep mosquitto

# Check logs
docker logs mosquitto

# Restart Mosquitto
docker restart mosquitto
```

### Issue: Tests fail
```bash
# Run with output
cargo test -- --nocapture

# Run single test
cargo test test_mqtt_to_parquet_flow -- --nocapture

# Clean and rebuild
cargo clean
cargo build
cargo test
```

### Issue: Parquet files not created
```bash
# Check storage path exists
ls -la /data/parquet

# Create if needed
mkdir -p /data/parquet

# Check permissions
ls -ld /data/parquet

# Check logs for storage errors
cargo run --bin air-quality-app 2>&1 | grep -i storage
```

---

## Getting Help

### Resources
- **Specification:** `/product/features/air-002/specs/01-specification.md`
- **Roadmap:** `/product/features/air-002/implementation/01-roadmap.md`
- **Git Workflow:** `/product/features/air-002/implementation/06-git-workflow.md`

### Memory Retrieval
```bash
# Retrieve requirements
npx claude-flow@alpha memory retrieve "air002/requirements"

# Retrieve git plan
npx claude-flow@alpha memory retrieve "air002/git_plan"
```

---

## Success Criteria

When you're done, you should have:
- [ ] All 7 commits pushed to `feature/air-002-ingestion-pipeline`
- [ ] All tests passing (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Manual verification completed
- [ ] Documentation written
- [ ] Pull request created
- [ ] No mock implementations remaining in code

---

**Ready to Start?**
1. Create the feature branch
2. Start with Task 1 (Configuration)
3. Work through tasks in order
4. Commit and push after each task
5. Create PR when all tasks complete

**Estimated Timeline:** 3-4 days
**Good luck!**
