# Testing Checklist - Air Quality App Config Fixes

**Date**: 2025-12-14
**Branch**: feature/air-001-implementation
**Status**: Ready for Testing

---

## Changes Summary

DevOps agent fixed **3 critical configuration inconsistencies**:
1. ✅ Environment variable name in docker-compose.yml (`DATA_DIR` → `STORAGE_PATH`)
2. ✅ Storage path in base config.yaml (`/data/parquet` → `/app/data`)
3. ✅ Storage path in production overlay (`/var/data/...` → `/app/data`)

Developer must complete:
- ⏳ Integration of etcd config loading in main.rs

---

## Test Scenarios

### Scenario 1: Basic Deployment with Environment Variable Override

**Purpose**: Verify that STORAGE_PATH env var is correctly read by the app

**Steps**:
```bash
cd /workspaces/neural-data-platform/deploy/pi

# 1. Start services
docker compose up -d

# 2. Wait for startup
sleep 10

# 3. Check app logs for config loading
docker logs air-quality-app | grep -i "storage\|config"
```

**Expected Output**:
```
INFO Loaded configuration from config.yaml
INFO Initializing ParquetStore at: /app/data
```

**Validation**:
- [ ] App starts successfully
- [ ] Logs show storage path: `/app/data`
- [ ] No errors about missing directories

---

### Scenario 2: Data Persistence Across Restarts

**Purpose**: Verify data is written to mounted volume and persists

**Steps**:
```bash
# 1. Ensure app is running
docker ps | grep air-quality-app

# 2. Trigger data ingestion (send MQTT message)
docker exec mosquitto mosquitto_pub \
  -t "airgradient/readings/test-sensor-01" \
  -m '{"pm25": 12.5, "co2": 450, "temp": 22.0, "humidity": 45, "timestamp": "2025-12-14T15:00:00Z"}'

# 3. Wait for batch write (5 seconds)
sleep 6

# 4. Check if data files exist
docker exec air-quality-app ls -la /app/data/

# 5. Note file count
FILE_COUNT_1=$(docker exec air-quality-app find /app/data -type f | wc -l)
echo "Files before restart: $FILE_COUNT_1"

# 6. Restart container
docker compose restart air-quality-app
sleep 10

# 7. Check if data still exists
FILE_COUNT_2=$(docker exec air-quality-app find /app/data -type f | wc -l)
echo "Files after restart: $FILE_COUNT_2"

# 8. Verify counts match
if [ "$FILE_COUNT_1" -eq "$FILE_COUNT_2" ]; then
  echo "✅ Data persisted across restart"
else
  echo "❌ Data lost on restart"
fi
```

**Expected Results**:
- [ ] Data files created in `/app/data/`
- [ ] File count stays same after restart
- [ ] Data is readable after restart

---

### Scenario 3: Volume Mount Verification

**Purpose**: Verify docker volume is correctly mounted

**Steps**:
```bash
# 1. Check docker volume exists
docker volume ls | grep air-quality-data

# 2. Inspect volume mount in container
docker inspect air-quality-app | grep -A 10 "Mounts"

# 3. Verify mount point
docker exec air-quality-app mount | grep /app/data

# 4. Test write permissions
docker exec air-quality-app touch /app/data/test-write.txt
docker exec air-quality-app rm /app/data/test-write.txt
```

**Expected Results**:
- [ ] Volume `air-quality-data` exists
- [ ] Mounted at `/app/data` in container
- [ ] Write permissions are correct
- [ ] Can create and delete test files

---

### Scenario 4: Config Sync to etcd

**Purpose**: Verify configs are correctly synced to etcd

**Steps**:
```bash
cd /workspaces/neural-data-platform

# 1. Sync base config to etcd
./scripts/sync-config-to-etcd.sh development

# 2. Verify storage path in etcd
docker exec etcd etcdctl get /air-quality/storage/base_path

# 3. Sync production config to etcd
./scripts/sync-config-to-etcd.sh production

# 4. Verify production storage path
docker exec etcd etcdctl get /air-quality/storage/base_path

# 5. List all storage config keys
docker exec etcd etcdctl get --prefix /air-quality/storage/
```

**Expected Results**:
```
# Base config
/air-quality/storage/base_path = "/app/data"
/air-quality/storage/wal_enabled = true
/air-quality/storage/batch_size = 100
/air-quality/storage/batch_timeout_secs = 5

# Production config (after second sync)
/air-quality/storage/base_path = "/app/data"
/air-quality/storage/batch_size = 500
/air-quality/storage/batch_timeout_secs = 10
```

**Validation**:
- [ ] etcd contains correct storage path (`/app/data`)
- [ ] Production override applied (batch_size = 500)
- [ ] No old incorrect paths remain

---

### Scenario 5: Environment Variable Override Test

**Purpose**: Verify env vars override YAML config

**Steps**:
```bash
cd /workspaces/neural-data-platform/deploy/pi

# 1. Stop current deployment
docker compose down

# 2. Edit docker-compose.yml to add test override
# Add: - STORAGE_PATH=/tmp/test-override

# 3. Start with override
docker compose up -d
sleep 10

# 4. Check logs for storage path
docker logs air-quality-app | grep "Initializing ParquetStore"

# 5. Verify app uses override path
docker exec air-quality-app ls -la /tmp/test-override

# 6. Cleanup - revert docker-compose.yml
# Remove: - STORAGE_PATH=/tmp/test-override
docker compose down
docker compose up -d
```

**Expected Results**:
- [ ] App uses `/tmp/test-override` when env var set
- [ ] App creates directory if it doesn't exist
- [ ] App falls back to config value when env var removed

---

### Scenario 6: YAML Config Fallback Test

**Purpose**: Verify app loads from YAML when etcd unavailable

**Steps**:
```bash
cd /workspaces/neural-data-platform/deploy/pi

# 1. Stop etcd service
docker compose stop etcd

# 2. Restart app (should fall back to YAML)
docker compose restart air-quality-app
sleep 10

# 3. Check logs for fallback behavior
docker logs air-quality-app | grep -i "etcd\|config\|fallback"

# 4. Verify app still works
curl http://localhost:8080/health

# 5. Restart etcd
docker compose start etcd
docker compose restart air-quality-app
```

**Expected Results** (current implementation):
- [ ] App starts successfully without etcd
- [ ] Uses config.yaml (no etcd warning since not yet implemented)
- [ ] Health endpoint responds

**Expected Results** (after developer integration):
- [ ] App logs warning about etcd unavailable
- [ ] App falls back to YAML config
- [ ] App still fully functional

---

### Scenario 7: Multi-Container Restart Test

**Purpose**: Verify data survives full stack restart

**Steps**:
```bash
cd /workspaces/neural-data-platform/deploy/pi

# 1. Send test data
docker exec mosquitto mosquitto_pub \
  -t "airgradient/readings/sensor-01" \
  -m '{"pm25": 15.0, "co2": 500}'

sleep 6

# 2. Count files
COUNT_BEFORE=$(docker exec air-quality-app find /app/data -type f -name "*.parquet" | wc -l)
echo "Parquet files before: $COUNT_BEFORE"

# 3. Full stack restart
docker compose down
docker compose up -d
sleep 15

# 4. Count files again
COUNT_AFTER=$(docker exec air-quality-app find /app/data -type f -name "*.parquet" | wc -l)
echo "Parquet files after: $COUNT_AFTER"

# 5. Compare
if [ "$COUNT_BEFORE" -eq "$COUNT_AFTER" ]; then
  echo "✅ Data survived full restart"
else
  echo "❌ Data lost on restart"
fi
```

**Expected Results**:
- [ ] All services restart successfully
- [ ] Parquet file count remains same
- [ ] Data is intact and readable

---

## Edge Cases to Test

### Edge Case 1: Empty Data Directory
```bash
# Start with empty volume
docker compose down -v
docker compose up -d

# Verify app creates directory structure
docker exec air-quality-app ls -la /app/data
```

**Expected**: App creates necessary directories

---

### Edge Case 2: Invalid Config Path
```bash
# Set invalid storage path
# Edit docker-compose: STORAGE_PATH=/invalid/path

docker compose up -d

# Check logs for error handling
docker logs air-quality-app | grep -i error
```

**Expected**: App logs error about invalid path or creates directory

---

### Edge Case 3: Permission Issues
```bash
# Make volume read-only (test)
docker exec air-quality-app chmod 444 /app/data

# Send data
docker exec mosquitto mosquitto_pub -t "test" -m '{}'

# Check logs
docker logs air-quality-app | tail -20
```

**Expected**: App logs permission error gracefully

---

## Regression Tests

### Test Previous Bug (Volume Mismatch)

**Before Fix**: App wrote to `/data/parquet` (unmounted)
**After Fix**: App writes to `/app/data` (mounted)

```bash
# Verify app never tries to write to old path
docker compose up -d
sleep 10

# Old path should not exist or be empty
docker exec air-quality-app ls /data/parquet 2>&1

# New path should have data
docker exec air-quality-app ls -la /app/data
```

**Expected**:
- [ ] `/data/parquet` doesn't exist or is empty
- [ ] `/app/data` contains parquet files

---

## Performance Tests

### Test Data Ingestion Rate
```bash
# Send 100 messages rapidly
for i in {1..100}; do
  docker exec mosquitto mosquitto_pub \
    -t "airgradient/readings/sensor-$i" \
    -m "{\"pm25\": $i, \"co2\": 400}"
done

# Wait for batching
sleep 10

# Count records written
docker exec air-quality-app du -sh /app/data
```

**Expected**: All messages processed and written

---

## Health Checks

### Service Health
```bash
# Check all services are healthy
docker compose ps

# Expected output:
# mosquitto       healthy
# etcd            healthy
# air-quality-app healthy (after 30s)
```

### API Health Endpoint
```bash
# Test health endpoint
curl -s http://localhost:8080/health | jq .

# Expected:
# {
#   "status": "healthy",
#   "storage": "operational",
#   "mqtt": "connected"
# }
```

---

## Cleanup After Testing

```bash
# Stop all services
docker compose down

# Remove volumes (optional - for fresh start)
docker compose down -v

# Remove test data
docker volume rm air-quality-data
```

---

## Test Result Template

```markdown
## Test Results - Air Quality Config Fixes

**Tester**: [Your Name]
**Date**: 2025-12-14
**Branch**: feature/air-001-implementation

### Scenario 1: Basic Deployment
- [ ] PASS / [ ] FAIL
- Notes: ___________________________

### Scenario 2: Data Persistence
- [ ] PASS / [ ] FAIL
- Notes: ___________________________

### Scenario 3: Volume Mount
- [ ] PASS / [ ] FAIL
- Notes: ___________________________

### Scenario 4: Config Sync
- [ ] PASS / [ ] FAIL
- Notes: ___________________________

### Scenario 5: Env Override
- [ ] PASS / [ ] FAIL
- Notes: ___________________________

### Scenario 6: YAML Fallback
- [ ] PASS / [ ] FAIL
- Notes: ___________________________

### Scenario 7: Multi-Container Restart
- [ ] PASS / [ ] FAIL
- Notes: ___________________________

### Overall Status
- [ ] All tests passed - Ready for merge
- [ ] Some tests failed - See notes
- [ ] Blocked - Waiting for developer

### Issues Found
1. ___________________________
2. ___________________________
```

---

## Dependencies

### Required Services
- Docker & Docker Compose
- Running etcd instance
- Running mosquitto broker

### Required Tools
- `docker` CLI
- `jq` (for JSON parsing)
- `curl` (for HTTP tests)

---

## Success Criteria

**All tests must pass**:
- ✅ App reads `STORAGE_PATH` environment variable
- ✅ App writes to `/app/data` (mounted volume)
- ✅ Data persists across container restarts
- ✅ Config syncs correctly to etcd
- ✅ Environment variables override config
- ✅ No data written to unmounted paths

**Blockers for merge**:
- ❌ Data not persisting
- ❌ App writing to wrong path
- ❌ Config sync failing

---

## Contact

**DevOps Agent**: Configuration fixes complete
**Developer Agent**: etcd integration pending
**Tester Agent**: Use this checklist for validation

---

**End of Checklist**
