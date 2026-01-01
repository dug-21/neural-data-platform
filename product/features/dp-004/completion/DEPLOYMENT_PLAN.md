# DP-004: Deployment Plan

## Overview

This document outlines the deployment strategy for the Bronze Raw JSON Schema feature.

> **Simplified Approach**: Platform is <1 week old. No backward compatibility required.
> Existing data can be retired. Clean cutover to new schema.

## Deployment Scope

| Component | Deployment Target | Impact Level |
|-----------|------------------|--------------|
| neural-core library | All consumers | High |
| air-quality-app | Raspberry Pi | High |
| Parquet storage | /data/bronze/* | Medium |
| Configuration | etcd + local | Low |

---

## Pre-Deployment Verification

### 1. Build Verification

```bash
# Clean build
cargo clean && cargo build --release

# Run full test suite
cargo test --all

# Check for warnings
cargo clippy --all-targets --all-features -- -D warnings

# Verify formatting
cargo fmt --all -- --check
```

### 2. Configuration Validation

```bash
# Validate all stream configurations
./deploy/pi/deploy.sh validate-config

# Check etcd connectivity
etcdctl get --prefix /ndp/streams/
```

### 3. Storage Capacity Check

```bash
# Verify disk space
df -h /data/

# Current Bronze size (will be retired)
du -sh /data/bronze/
```

---

## Deployment Steps

### Step 1: Stop Services

```bash
# Stop current ingestion
./deploy/pi/deploy.sh stop

# Verify stopped
./deploy/pi/deploy.sh status
```

### Step 2: Clear Old Data (Optional)

```bash
# Archive old data if needed for reference
mkdir -p /data/archive
mv /data/bronze /data/archive/bronze-v1-$(date +%Y%m%d)

# Or simply remove (platform is <1 week old)
rm -rf /data/bronze/*
```

### Step 3: Deploy New Version

```bash
# Sync configuration
./deploy/pi/deploy.sh sync

# Start with new version
./deploy/pi/deploy.sh start

# Verify running
./deploy/pi/deploy.sh status
```

### Step 4: Verify New Schema

```bash
# Wait for first data
sleep 60

# Check new Parquet schema
duckdb -c "
SELECT * FROM parquet_schema('/data/bronze/*.parquet');
"

# Should show: timestamp, source_id, ndp_id, context, raw_payload

# Verify raw_payload contains JSON
duckdb -c "
SELECT timestamp, source_id,
       json_extract_string(raw_payload, '\$.pm02') as pm02
FROM read_parquet('/data/bronze/*.parquet')
LIMIT 5;
"
```

---

## Rollback Strategy

### Trigger Conditions

Initiate rollback if ANY of the following occur:

1. **Data Loss**: Records not appearing in Parquet files
2. **Parsing Errors**: >1% of messages failing to process
3. **Application Crash**: Repeated OOM or panic errors

### Rollback Procedure

```bash
# Stop application
./deploy/pi/deploy.sh stop

# Revert to previous binary
cd /opt/ndp
mv air-quality-app air-quality-app-dp004
mv air-quality-app-backup air-quality-app

# Revert configuration
etcdctl restore /data/backup/etcd-pre-dp004.json

# Start with old version
./deploy/pi/deploy.sh start
```

---

## Monitoring Requirements

### Metrics to Track

| Metric | Tool | Alert Threshold |
|--------|------|-----------------|
| Ingestion rate | Prometheus | < 80% of baseline |
| Storage growth | Node Exporter | > 150% expected rate |
| Parse errors | Application logs | > 1% of messages |
| Memory usage | Node Exporter | > 80% available |

### Log Monitoring

```bash
# Watch for errors during deployment
journalctl -u air-quality -f | grep -E "(ERROR|PANIC|WARN)"

# Check Parquet write activity
ls -la /data/bronze/ | head -20
```

---

## Success Metrics

### Immediate (Day 1)

- [ ] Application starts without errors
- [ ] All sources emitting RawDataPoint
- [ ] Parquet files using new 5-column schema
- [ ] No increase in error rate

### Short-term (Week 1)

- [ ] Storage growth within expectations
- [ ] DuckDB JSON queries working
- [ ] All streams processing correctly

---

## Environment-Specific Notes

### Raspberry Pi Deployment

```bash
# SSH to Pi
ssh ndp@raspberrypi.local

# Check system resources
free -m
df -h /data

# Deployment
cd /opt/ndp
./deploy.sh stop
./deploy.sh update
./deploy.sh start
./deploy.sh logs
```

### Development Environment

```bash
# Local testing
cargo run --bin air-quality-app -- --config config/dev.yaml

# Verify Parquet output
duckdb -c "SELECT * FROM read_parquet('./data/bronze/*.parquet') LIMIT 5;"
```

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-01 | ndp-scrum-master | Initial draft |
| 1.1 | 2026-01-01 | ndp-scrum-master | Simplified: no backward compat needed |
