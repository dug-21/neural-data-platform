# AIR-004: Implementation Guide

**Task:** Integrate StreamRegistry into air-quality-app startup
**Complexity:** Low (10 lines of code)
**Time Estimate:** 30 minutes
**Risk:** Minimal (additive only, no breaking changes)

## Quick Start

### What You're Doing

Adding **optional** StreamConfig loading during app startup. The app will:
1. Try to load `/streams/air-quality/config` from etcd
2. Log the schema if found (for validation)
3. Continue normally whether found or not

**No behavior changes.** Just observability.

## Code Changes

### File: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

**Location:** After `let config = ...` block (around line 65)

**Add this code:**

```rust
// ============================================================================
// Phase 1: StreamRegistry Integration (AIR-004)
// ============================================================================
// Load StreamConfig from registry if available. This is additive - existing
// /air-quality/* etcd paths continue to work. Future phases will use
// StreamConfig to drive handler creation.
// ============================================================================

use config_client::StreamRegistry;

let etcd_endpoint = std::env::var("ETCD_ENDPOINT")
    .unwrap_or_else(|_| "http://localhost:2379".to_string());

match StreamRegistry::new(&[&etcd_endpoint]).await {
    Ok(registry) => {
        match registry.load_stream("air-quality").await {
            Ok(stream_config) => {
                tracing::info!(
                    "StreamConfig loaded for 'air-quality': \
                     stream_id={}, version={}, fields={}, sources={}",
                    stream_config.stream_id,
                    stream_config.version,
                    stream_config.fields.len(),
                    stream_config.sources.len()
                );

                // Log field schema for verification
                for field in &stream_config.fields {
                    tracing::debug!(
                        "  Field: {} ({:?}) - unit: {:?}, nullable: {}",
                        field.name,
                        field.field_type,
                        field.unit,
                        field.nullable
                    );
                }

                // Validate StreamConfig
                if let Err(e) = stream_config.validate() {
                    tracing::warn!(
                        "StreamConfig validation failed (will use legacy config): {}",
                        e
                    );
                }

                // Future (AIR-005): Use stream_config to create handlers
                // For now, this is informational only
            }
            Err(e) => {
                tracing::info!(
                    "No StreamConfig found in registry (this is expected for \
                     existing deployments that use /air-quality/* etcd paths): {}",
                    e
                );
                tracing::info!(
                    "Future: Create StreamConfig from AppConfig using migration tool"
                );
            }
        }
    }
    Err(e) => {
        tracing::warn!(
            "Failed to initialize StreamRegistry (etcd may be unavailable): {}. \
             Continuing with legacy config.",
            e
        );
    }
}

// ============================================================================
// End of StreamRegistry Integration
// ============================================================================
```

**That's it!** The rest of main.rs stays exactly the same.

## Testing

### Test 1: Existing Deployment (No StreamConfig)

**Setup:**
- Deploy to environment with `/air-quality/*` etcd config
- No `/streams/air-quality/config` exists

**Expected Behavior:**
```
INFO  Loaded configuration from etcd
INFO  No StreamConfig found in registry (this is expected for existing deployments...)
INFO  Future: Create StreamConfig from AppConfig using migration tool
INFO  Starting air quality server on 0.0.0.0:8080
INFO  Initializing ParquetStore at: /app/data
INFO  MQTT handler initialized successfully
```

**Verify:**
- [ ] App starts successfully
- [ ] MQTT handler connects
- [ ] Data ingestion works
- [ ] ParquetStore writes data

### Test 2: New Deployment (With StreamConfig)

**Setup:**
1. Create StreamConfig in etcd:
   ```bash
   # Use the example from docs/air-quality-stream-config-example.json
   etcdctl put /streams/air-quality/config "$(cat docs/air-quality-stream-config-example.json)"
   ```

**Expected Behavior:**
```
INFO  Loaded configuration from etcd
INFO  StreamConfig loaded for 'air-quality': stream_id=air-quality, version=1.0.0, fields=10, sources=1
DEBUG Field: pm25 (Float) - unit: Some("µg/m³"), nullable: false
DEBUG Field: pm10 (Float) - unit: Some("µg/m³"), nullable: true
DEBUG Field: co2 (Int) - unit: Some("ppm"), nullable: true
...
INFO  Starting air quality server on 0.0.0.0:8080
```

**Verify:**
- [ ] App starts successfully
- [ ] StreamConfig details logged
- [ ] Field schema logged (10 fields)
- [ ] MQTT handler connects
- [ ] Data ingestion works

### Test 3: Invalid StreamConfig

**Setup:**
1. Create invalid StreamConfig:
   ```bash
   etcdctl put /streams/air-quality/config '{"stream_id":"air-quality","fields":[],"sources":[]}'
   ```

**Expected Behavior:**
```
INFO  StreamConfig loaded for 'air-quality': stream_id=air-quality, version=1.0.0, fields=0, sources=0
WARN  StreamConfig validation failed (will use legacy config): Stream must have at least one field
INFO  Starting air quality server on 0.0.0.0:8080
```

**Verify:**
- [ ] App starts successfully (uses legacy config)
- [ ] Validation error logged
- [ ] MQTT handler uses legacy config
- [ ] Data ingestion works

### Test 4: etcd Down

**Setup:**
- Stop etcd before starting app

**Expected Behavior:**
```
WARN  Failed to connect to etcd: connection refused. Falling back to file config.
WARN  Failed to load config from etcd: ...
INFO  Loaded configuration from config.yaml
WARN  Failed to initialize StreamRegistry (etcd may be unavailable): connection refused. Continuing with legacy config.
INFO  Starting air quality server on 0.0.0.0:8080
```

**Verify:**
- [ ] App starts with YAML fallback
- [ ] No crashes or panics
- [ ] App works in degraded mode

## Deployment Steps

### Development Environment

```bash
# 1. Make code changes in main.rs
vim apps/air-quality-app/src/main.rs

# 2. Build
cargo build --release

# 3. Test locally (no StreamConfig)
./target/release/air-quality-server

# 4. Create StreamConfig in etcd
etcdctl put /streams/air-quality/config "$(cat docs/air-quality-stream-config-example.json)"

# 5. Restart and verify logs
./target/release/air-quality-server

# 6. Check registry
etcdctl get /streams/air-quality/config
```

### Production (Raspberry Pi)

```bash
# 1. Update code in repo
git add apps/air-quality-app/src/main.rs
git commit -m "feat(air-quality): integrate StreamRegistry for multi-stream support (AIR-004)"
git push

# 2. SSH to Pi
ssh pi@10.0.0.100

# 3. Pull changes
cd ~/neural-data-platform
git pull

# 4. Rebuild Docker image
docker compose build air-quality-app

# 5. Restart service
docker compose restart air-quality-app

# 6. Check logs
docker compose logs -f air-quality-app

# Expected: "No StreamConfig found in registry (this is expected...)"
```

## Verification Checklist

After deployment, verify:

- [ ] Application starts successfully
- [ ] Logs show registry initialization attempt
- [ ] MQTT connection established
- [ ] Data ingestion continues
- [ ] ParquetStore receives data
- [ ] HTTP API responds
- [ ] No errors or panics
- [ ] Performance unchanged

## Rollback Plan

If something goes wrong:

```bash
# Quick rollback
git revert HEAD
git push
docker compose build air-quality-app
docker compose restart air-quality-app

# Or: Comment out StreamRegistry code block
# The app will work exactly as before
```

**Recovery Time:** <5 minutes

## Common Issues

### Issue 1: StreamRegistry Import Error

**Error:**
```
error[E0432]: unresolved import `config_client::StreamRegistry`
```

**Solution:**
Check that `config-client` is in Cargo.toml (it should be):
```toml
config-client = { path = "../../config-client" }
```

### Issue 2: StreamConfig Not Found

**Behavior:**
```
INFO No StreamConfig found in registry...
```

**Expected:** This is normal for existing deployments. The app uses legacy `/air-quality/*` paths.

**To Add StreamConfig:**
```bash
etcdctl put /streams/air-quality/config "$(cat docs/air-quality-stream-config-example.json)"
```

### Issue 3: Validation Fails

**Behavior:**
```
WARN StreamConfig validation failed: Invalid stream ID: air_quality
```

**Solution:**
- stream_id must be kebab-case (not snake_case)
- Fix: `"stream_id": "air-quality"`
- Field names must be snake_case
- Must have at least one field and one source

### Issue 4: etcd Connection Fails

**Behavior:**
```
WARN Failed to initialize StreamRegistry (etcd may be unavailable)
```

**Expected:** App continues with legacy config or YAML fallback.

**Check:**
```bash
# Verify etcd is running
docker compose ps etcd

# Test connection
etcdctl endpoint health

# Check ETCD_ENDPOINT env var
echo $ETCD_ENDPOINT
```

## Success Metrics

After implementation, you should see:

1. **Logs show integration:**
   - "StreamConfig loaded..." (if config exists)
   - OR "No StreamConfig found..." (if not exists)

2. **No regressions:**
   - MQTT ingestion works
   - Data written to Parquet
   - API endpoints respond
   - No performance degradation

3. **Foundation for future:**
   - Registry can load stream configs
   - Validation works
   - Ready for AIR-005 (handler factory)

## Next Steps After AIR-004

Once this is deployed and verified:

1. **AIR-005: Generic Handler Factory**
   - Use StreamConfig to create MQTT/HTTP handlers
   - Dynamic ParquetStore schema from fields
   - Stream multiplexing

2. **AIR-006: Migration Tool**
   - CLI: `air-quality-migrate --from-legacy air-quality`
   - Converts AppConfig → StreamConfig
   - Saves to etcd

3. **AIR-007: Auto-Generated APIs**
   - `/api/streams/{id}/data`
   - `/api/streams/{id}/schema`
   - Discovery endpoint: `/api/streams`

## Questions?

**Why not use StreamConfig immediately?**
- Minimizes risk (additive only)
- Tests integration without breaking changes
- Allows gradual migration

**When will StreamConfig actually be used?**
- AIR-005 will use it to create handlers
- AIR-006 provides migration tooling
- AIR-007 adds auto-generated endpoints

**What if I want to add a new stream now?**
- Create StreamConfig JSON (see example)
- Save to `/streams/{stream-id}/config`
- Registry will discover it
- Handler creation manual until AIR-005

---

**Implementation Time:** 30 minutes
**Testing Time:** 15 minutes
**Total:** <1 hour from start to deployed

**Risk Level:** LOW ✅
**Breaking Changes:** NONE ✅
**Rollback Time:** <5 minutes ✅

Good luck! 🚀
