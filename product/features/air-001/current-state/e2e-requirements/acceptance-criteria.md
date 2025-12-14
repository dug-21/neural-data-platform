# E2E Test Acceptance Criteria

**Version:** 1.0.0
**Date:** December 14, 2025
**Purpose:** Success criteria for E2E testing of Air Quality Platform

---

## Minimum Viable E2E (MVP)

The following criteria MUST pass before declaring E2E test capability:

### AC-1: Data Flow

| ID | Criterion | Validation Method | Pass/Fail |
|----|-----------|-------------------|-----------|
| AC-1.1 | MQTT message received within 1s of publish | Timestamp comparison | |
| AC-1.2 | Reading stored in Parquet within 5s | File existence check | |
| AC-1.3 | Reading queryable via REST API | GET /api/v1/readings/latest returns data | |
| AC-1.4 | 100 consecutive readings processed without error | Counter validation | |

### AC-2: Data Persistence

| ID | Criterion | Validation Method | Pass/Fail |
|----|-----------|-------------------|-----------|
| AC-2.1 | Data survives application restart | Query after restart | |
| AC-2.2 | WAL replay recovers in-flight data | Kill -9 + restart | |
| AC-2.3 | Daily partitions created correctly | Directory structure check | |

### AC-3: Health Monitoring

| ID | Criterion | Validation Method | Pass/Fail |
|----|-----------|-------------------|-----------|
| AC-3.1 | Health endpoint returns 200 when healthy | GET /health | |
| AC-3.2 | Health degrades when MQTT disconnects | Stop broker + check | |
| AC-3.3 | last_reading_age_seconds is accurate | Compare with wall clock | |

### AC-4: Docker Deployment

| ID | Criterion | Validation Method | Pass/Fail |
|----|-----------|-------------------|-----------|
| AC-4.1 | docker compose up starts all services | Container status | |
| AC-4.2 | Health checks pass within 60s | Docker health status | |
| AC-4.3 | Volumes persist data across restarts | Query after down/up | |
| AC-4.4 | Graceful shutdown completes within 30s | docker compose down timing | |

---

## Full E2E Criteria

The following criteria represent complete air-001 specification compliance:

### AC-5: Alerting

| ID | Criterion | Validation Method | Pass/Fail |
|----|-----------|-------------------|-----------|
| AC-5.1 | CO2 > 1000 ppm generates Moderate alert | Inject high CO2 reading | |
| AC-5.2 | PM2.5 > 35 µg/m³ generates Unhealthy alert | Inject high PM2.5 | |
| AC-5.3 | Alerts queryable via REST API | GET /api/v1/alerts | |
| AC-5.4 | Alert deduplication prevents storms | Send 10 violations, expect 1 alert | |

### AC-6: Forecasting

| ID | Criterion | Validation Method | Pass/Fail |
|----|-----------|-------------------|-----------|
| AC-6.1 | Model loads within 30s (cold start) | First request timing | |
| AC-6.2 | Inference completes within 2s (warm) | Subsequent request timing | |
| AC-6.3 | Forecasts include p10/p50/p90 intervals | Response schema validation | |
| AC-6.4 | 6-hour horizon returns 72 predictions | Count predictions | |

### AC-7: Query Performance

| ID | Criterion | Validation Method | Pass/Fail |
|----|-----------|-------------------|-----------|
| AC-7.1 | 24-hour query completes in <100ms | Request timing | |
| AC-7.2 | Aggregation returns correct values | Compare with manual calculation | |
| AC-7.3 | Empty time range returns empty array | Query future dates | |

### AC-8: MCP Integration

| ID | Criterion | Validation Method | Pass/Fail |
|----|-----------|-------------------|-----------|
| AC-8.1 | 5 tools discoverable via MCP | List tools request | |
| AC-8.2 | air_quality_query returns current data | Tool invocation | |
| AC-8.3 | air_quality_forecast returns predictions | Tool invocation | |
| AC-8.4 | air_quality_recommendations contextual | Tool invocation | |

---

## Performance Criteria

### Response Time Limits

| Endpoint | P50 | P95 | P99 |
|----------|-----|-----|-----|
| GET /health | <10ms | <50ms | <100ms |
| GET /api/v1/readings/latest | <20ms | <50ms | <100ms |
| GET /api/v1/readings (24h) | <50ms | <100ms | <200ms |
| GET /api/v1/aggregate | <50ms | <100ms | <200ms |
| GET /api/v1/forecast | <2000ms | <5000ms | <10000ms |

### Throughput Limits

| Metric | Minimum | Target |
|--------|---------|--------|
| Readings/second | 1 | 10 |
| Concurrent queries | 5 | 20 |
| Storage efficiency | <1.5MB/day | <1MB/day |

### Resource Limits

| Resource | Development | Production (Pi5) |
|----------|-------------|------------------|
| Memory | <1GB | <2GB |
| CPU | <2 cores | <2 cores |
| Disk I/O | <10MB/s | <5MB/s |

---

## Reliability Criteria

### Uptime

| Scenario | Criterion |
|----------|-----------|
| Normal operation | 99.9% uptime (8.7 hours downtime/year) |
| Network partition | Automatic recovery within 60s |
| Broker restart | Reconnect within 30s |
| Application crash | Restart within 10s (Docker) |

### Data Integrity

| Scenario | Criterion |
|----------|-----------|
| Normal shutdown | 100% data persisted |
| Kill -9 crash | <5 seconds data loss (WAL) |
| Disk full | Graceful degradation, alert |
| Corrupted WAL entry | Skip and continue |

---

## Test Execution Checklist

### Pre-Test Setup
- [ ] Docker environment clean (`docker compose down -v`)
- [ ] Test data prepared (mock sensor readings)
- [ ] Monitoring enabled (Prometheus/Grafana optional)
- [ ] Test runner configured

### Smoke Test (5 minutes)
- [ ] AC-4.1: All containers start
- [ ] AC-4.2: Health checks pass
- [ ] AC-3.1: Health endpoint 200
- [ ] AC-1.1: First message received

### Functional Test (15 minutes)
- [ ] AC-1.1 through AC-1.4: Data flow complete
- [ ] AC-2.1 through AC-2.3: Persistence verified
- [ ] AC-3.1 through AC-3.3: Health accurate

### Integration Test (20 minutes)
- [ ] AC-5.1 through AC-5.4: Alerting works
- [ ] AC-6.1 through AC-6.4: Forecasting works
- [ ] AC-7.1 through AC-7.3: Queries performant

### Performance Test (10 minutes)
- [ ] Response times within limits
- [ ] Throughput meets minimum
- [ ] Resources within limits

### Post-Test Cleanup
- [ ] Test results exported
- [ ] Logs collected
- [ ] Containers stopped
- [ ] Volumes cleaned (if needed)

---

## Failure Handling

### On Test Failure

1. **Capture Logs**
   ```bash
   docker compose logs > test-failure-logs.txt
   ```

2. **Capture Metrics**
   ```bash
   curl http://localhost:9090/api/v1/query?query=up > metrics.json
   ```

3. **Preserve State**
   ```bash
   docker compose exec air-quality-app cat /data/wal/*.wal > wal-dump.txt
   ```

4. **File Issue**
   - Include test ID that failed
   - Attach logs and metrics
   - Describe expected vs actual behavior

### Known Issues / Workarounds

| Issue | Workaround |
|-------|------------|
| MQTT connection timeout | Increase start_period in health check |
| Parquet write delay | Wait 10s after last message before query |
| Model load slow on first request | Pre-warm with dummy forecast |

---

## Sign-Off

### E2E Test Approval

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Developer | | | |
| QA Lead | | | |
| Product Owner | | | |

### Criteria Summary

| Category | Passing | Total | Percentage |
|----------|---------|-------|------------|
| MVP (AC-1 to AC-4) | | 14 | % |
| Full (AC-5 to AC-8) | | 16 | % |
| Performance | | 12 | % |
| Reliability | | 6 | % |
| **Total** | | **48** | **%** |

**E2E Status:** [ ] PASS / [ ] FAIL

**Notes:**
