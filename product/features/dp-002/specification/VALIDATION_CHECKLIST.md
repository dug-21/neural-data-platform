# DP-002 Validation Checklist

## Overview

This checklist provides manual validation steps for DP-002 deployment. Use this during:
- Pre-deployment readiness review
- Deployment verification
- Post-deployment validation
- Incident response and rollback

---

## Pre-Deployment Checks

Complete ALL items before beginning deployment.

### Code Readiness

- [ ] All unit tests pass locally: `cargo test --package platform-core`
- [ ] All app tests pass: `cargo test --package air-quality-app`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code formatted: `cargo fmt --check`
- [ ] No security vulnerabilities: `cargo audit`

### Configuration Readiness

- [ ] Entity schemas added to all 6 existing stream configs
- [ ] HomeAssistant stream config.yaml created
- [ ] TimescaleDB service added to docker-compose.yml
- [ ] DuckDB service removed from docker-compose.yml
- [ ] deploy.sh updated with sync-dictionary command
- [ ] Data dictionary schema SQL file ready

### Documentation Readiness

- [ ] HOW_TO_ADD_NEW_STREAM.md updated
- [ ] HOW_TO_ADD_NEW_SOURCE.md updated
- [ ] Entity schema format documented

### Environment Readiness

- [ ] Pi accessible via SSH
- [ ] Docker running on Pi
- [ ] Sufficient disk space on Pi (>2GB free)
- [ ] Network connectivity to external APIs (NWS, AirNow)
- [ ] Git repository up to date on Pi

---

## Deployment Verification Steps

Execute in order during deployment.

### Step 1: Pre-Deployment Backup

```bash
# On Pi
cd /home/pi/neural-data-platform

# Backup current state
./deploy/pi/deploy.sh stop
docker compose -f deploy/pi/docker-compose.yml ps
cp deploy/pi/docker-compose.yml deploy/pi/docker-compose.yml.bak
tar -czf /tmp/ndp-backup-$(date +%Y%m%d).tar.gz /data/bronze/
```

- [ ] Services stopped
- [ ] docker-compose.yml backed up
- [ ] Bronze data backed up

### Step 2: Pull and Apply Changes

```bash
# Pull latest code
git pull origin main

# Verify changes
git log --oneline -5
```

- [ ] Changes pulled successfully
- [ ] DP-002 commits visible in log

### Step 3: Start Core Services

```bash
./deploy/pi/deploy.sh start
```

Wait 60 seconds for stabilization.

- [ ] etcd healthy: `docker exec etcd etcdctl endpoint health`
- [ ] MQTT running: `docker ps | grep mosquitto`
- [ ] Air Quality App healthy: `curl -s http://localhost:8080/health`

### Step 4: Verify DuckDB Container Removed

```bash
docker ps | grep duckdb
# Should return nothing
```

- [ ] DuckDB container NOT running
- [ ] No startup errors related to DuckDB

### Step 5: Verify TimescaleDB Started

```bash
# Check container status
docker ps | grep timescaledb

# Test connection
docker exec timescaledb psql -U postgres -c "SELECT version();"

# Check memory usage
docker stats timescaledb --no-stream --format "{{.MemUsage}}"
```

- [ ] TimescaleDB container running
- [ ] psql connection successful
- [ ] Memory usage < 512MB

### Step 6: Sync Configuration

```bash
./deploy/pi/deploy.sh sync
./deploy/pi/deploy.sh list-streams
```

- [ ] Sync completed without error
- [ ] All 7 streams listed (6 existing + homeassistant)

### Step 7: Sync Data Dictionary

```bash
./deploy/pi/deploy.sh sync-dictionary
```

- [ ] Command completed successfully
- [ ] No error messages

### Step 8: Verify Data Dictionary

```bash
docker exec timescaledb psql -U postgres -d ndp -c \
  "SELECT stream_id, COUNT(*) as attributes FROM data_dictionary GROUP BY stream_id ORDER BY stream_id;"
```

- [ ] All 7 streams appear in output
- [ ] Each stream has attribute count > 0

### Step 9: Verify Bronze Ingestion Active

```bash
# Check recent files (within last 5 minutes)
find /data/bronze -name "*.parquet" -mmin -5 | head -5
```

- [ ] Recent Parquet files exist
- [ ] Multiple streams showing activity

### Step 10: Verify Grafana Dashboards

```bash
# Test DuckDB plugin query
curl -s http://localhost:3000/api/ds/query \
  -H "Content-Type: application/json" \
  -d '{"queries":[{"refId":"A","datasource":"DuckDB","rawSql":"SELECT 1"}]}'
```

- [ ] Grafana accessible at http://pi-ip:3000
- [ ] Air Quality dashboard loads
- [ ] Outdoor Conditions dashboard loads
- [ ] DuckDB datasource queries work

---

## Post-Deployment Validation

Complete within 1 hour of deployment.

### Data Flow Validation

- [ ] air-quality stream: New data arriving in Bronze
- [ ] outdoor-weather stream: New data arriving in Bronze
- [ ] outdoor-air-quality stream: New data arriving in Bronze
- [ ] nws-observations stream: New data arriving in Bronze
- [ ] nws-forecast-hourly stream: New data arriving in Bronze
- [ ] nws-gridpoints-forecast stream: New data arriving in Bronze

### Dashboard Validation

| Dashboard | Loads | Data Recent | Panels OK |
|-----------|-------|-------------|-----------|
| Air Quality | [ ] | [ ] | [ ] |
| Outdoor Conditions | [ ] | [ ] | [ ] |
| NWS Gridpoints Forecast | [ ] | [ ] | [ ] |
| Data Quality (NEW) | [ ] | [ ] | [ ] |

### Performance Validation

```bash
# System resources
free -h
df -h /data

# Container resources
docker stats --no-stream
```

- [ ] System memory usage < 6GB (on 8GB Pi)
- [ ] Disk usage < 80%
- [ ] No container restart loops

### Log Validation

```bash
./deploy/pi/deploy.sh logs 2>&1 | tail -100
```

- [ ] No ERROR level messages
- [ ] No PANIC messages
- [ ] No repeated failure patterns

---

## Rollback Procedures

Use if deployment fails validation.

### Quick Rollback (< 5 minutes)

For issues caught immediately:

```bash
# Stop services
./deploy/pi/deploy.sh stop

# Restore docker-compose.yml
cp deploy/pi/docker-compose.yml.bak deploy/pi/docker-compose.yml

# Reset to previous commit
git reset --hard HEAD~1

# Restart
./deploy/pi/deploy.sh start
```

### Full Rollback (< 15 minutes)

For data corruption or major issues:

```bash
# Stop services
./deploy/pi/deploy.sh stop

# Restore docker-compose.yml
cp deploy/pi/docker-compose.yml.bak deploy/pi/docker-compose.yml

# Reset to previous commit
git reset --hard HEAD~5  # Go back before DP-002 changes

# Restore Bronze data if needed
cd /data
tar -xzf /tmp/ndp-backup-YYYYMMDD.tar.gz

# Rebuild and restart
./deploy/pi/deploy.sh build
./deploy/pi/deploy.sh start
./deploy/pi/deploy.sh sync
```

### TimescaleDB-Specific Rollback

If only TimescaleDB is problematic:

```bash
# Stop TimescaleDB only
docker stop timescaledb
docker rm timescaledb

# Remove volume if data corrupted
docker volume rm pi_timescale_data

# Comment out TimescaleDB in docker-compose.yml
# Restart other services
./deploy/pi/deploy.sh start
```

---

## Sign-Off Criteria

### Deployment Approved When

All of the following are TRUE:

1. [ ] All Pre-Deployment Checks completed
2. [ ] All Deployment Verification Steps passed
3. [ ] All Post-Deployment Validation items confirmed
4. [ ] No rollback required
5. [ ] Monitoring shows stable operation for 30+ minutes

### Sign-Off Record

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Deployer | | | |
| Reviewer | | | |

---

## Incident Response

### Common Issues and Fixes

#### Issue: TimescaleDB won't start

**Symptoms**: Container exits immediately, no logs

**Fix**:
```bash
# Check logs
docker logs timescaledb

# If permission issue
sudo chown -R 1000:1000 /data/timescaledb

# Restart
docker start timescaledb
```

#### Issue: sync-dictionary fails

**Symptoms**: Error connecting to database

**Fix**:
```bash
# Check TimescaleDB is running
docker ps | grep timescaledb

# Check database exists
docker exec timescaledb psql -U postgres -c "\l"

# Create database if missing
docker exec timescaledb psql -U postgres -c "CREATE DATABASE ndp;"

# Retry sync
./deploy.sh sync-dictionary
```

#### Issue: Grafana shows "Datasource not found"

**Symptoms**: Panels error with datasource issues

**Fix**:
```bash
# Restart Grafana
docker restart grafana

# If persists, reprovision datasources
docker exec grafana grafana-cli plugins update-all
docker restart grafana
```

#### Issue: Bronze ingestion stopped

**Symptoms**: No new Parquet files

**Fix**:
```bash
# Check app logs
./deploy.sh logs air-quality-app

# Restart app
docker restart air-quality-app

# Verify sources reconnected
curl http://localhost:8080/health
```

---

## Appendix: Quick Reference Commands

### Service Health

```bash
# All services
docker ps

# Specific service logs
docker logs -f air-quality-app
docker logs -f timescaledb

# Health endpoints
curl http://localhost:8080/health
curl http://localhost:3000/api/health
```

### Data Dictionary

```bash
# List all streams
docker exec timescaledb psql -U postgres -d ndp -c \
  "SELECT DISTINCT stream_id FROM data_dictionary;"

# List attributes for stream
docker exec timescaledb psql -U postgres -d ndp -c \
  "SELECT attribute_name, attribute_type, unit FROM data_dictionary WHERE stream_id = 'air-quality';"
```

### Bronze Layer

```bash
# Recent files
ls -la /data/bronze/air-quality/*.parquet | tail -5

# File count by stream
for d in /data/bronze/*/; do echo "$d: $(ls -1 $d/*.parquet 2>/dev/null | wc -l) files"; done
```

---

## Related Documents

- [TEST_STRATEGY.md](./TEST_STRATEGY.md) - Test strategy
- [TEST_CASES.md](./TEST_CASES.md) - Detailed test cases
- [SCOPE.md](../SCOPE.md) - Feature scope
- [STATUS.md](../STATUS.md) - Implementation status

---

*This checklist ensures safe deployment and validation of DP-002. Follow all steps in order.*
