# DP-001 Completion Verification Plan

## Overview
This document specifies the verification criteria and procedures for confirming DP-001 implementation is complete. The plan validates that DuckDB and Grafana are properly integrated, Bronze layer data is accessible, Silver layer views are functional, and all dashboards render correctly with acceptable performance.

## 1. Pre-Deployment Checklist

### Configuration Files Exist
- [ ] `deploy/pi/docker-compose.yml` includes duckdb and grafana services
- [ ] `config/duckdb/init.sql` exists
- [ ] `config/duckdb/views/silver_indoor_air.sql` exists
- [ ] `config/duckdb/views/silver_outdoor_weather.sql` exists
- [ ] `config/duckdb/views/silver_outdoor_aqi.sql` exists
- [ ] `config/duckdb/views/cross_stream_aligned.sql` exists
- [ ] `config/grafana/grafana.ini` exists
- [ ] `config/grafana/provisioning/datasources/duckdb.yaml` exists
- [ ] `config/grafana/provisioning/dashboards/default.yaml` exists
- [ ] `config/grafana/dashboards/indoor_air.json` exists
- [ ] `config/grafana/dashboards/outdoor_weather.json` exists
- [ ] `config/grafana/dashboards/outdoor_aqi.json` exists
- [ ] `config/grafana/dashboards/indoor_outdoor_comparison.json` exists

### Configuration Validity
- [ ] `docker-compose.yml` syntax valid (run `docker-compose config`)
- [ ] SQL files have valid DuckDB syntax
- [ ] YAML files have valid syntax (run `yamllint config/`)
- [ ] JSON dashboards have valid Grafana schema

### File Permissions
- [ ] DuckDB data directory writable by container user
- [ ] Grafana config files readable by container user
- [ ] Parquet files readable by DuckDB container

## 2. Deployment Verification

### Container Startup
| Check | Command | Expected Result |
|-------|---------|-----------------|
| All containers running | `docker-compose ps` | 5 services UP (mqtt, coordinator, parquet-writer, duckdb, grafana) |
| DuckDB healthy | `docker inspect duckdb --format='{{.State.Health.Status}}'` | `healthy` |
| Grafana healthy | `docker inspect grafana --format='{{.State.Health.Status}}'` | `healthy` |
| No restart loops | `docker-compose logs --tail=50` | No repeated restart messages |
| Port bindings | `docker-compose ps` | DuckDB: none (internal), Grafana: 3000:3000 |

### Network Connectivity
| Check | Command | Expected Result |
|-------|---------|-----------------|
| Grafana accessible | `curl -s -o /dev/null -w "%{http_code}" http://localhost:3000` | `200` or `302` (redirect to login) |
| DuckDB internal | `docker exec grafana ping -c 1 duckdb` | Success (0% packet loss) |
| Volume mounts | `docker inspect duckdb --format='{{range .Mounts}}{{.Source}}:{{.Destination}}{{end}}'` | Includes `/data` mount |

### Log Health
- [ ] DuckDB logs show successful initialization
- [ ] DuckDB logs show Silver view creation
- [ ] Grafana logs show datasource provisioning success
- [ ] Grafana logs show dashboard provisioning success
- [ ] No ERROR level messages in startup logs

## 3. Functional Verification

### DuckDB Queries
| Test | Query | Expected |
|------|-------|----------|
| Parquet access | `SELECT COUNT(*) FROM read_parquet('/data/bronze/air-quality/*.parquet')` | Row count > 0 |
| Indoor air view | `SELECT * FROM silver_indoor_air LIMIT 1` | Valid row with timestamp, temp, humidity, co2 |
| Outdoor weather view | `SELECT * FROM silver_outdoor_weather LIMIT 1` | Valid row with timestamp, temp, humidity, pressure |
| Outdoor AQI view | `SELECT * FROM silver_outdoor_aqi LIMIT 1` | Valid row with timestamp, pm25, pm10, aqi |
| Cross-stream view | `SELECT * FROM cross_stream_aligned LIMIT 1` | Valid row joining indoor/outdoor data |
| Time filtering | `SELECT COUNT(*) FROM silver_indoor_air WHERE timestamp > NOW() - INTERVAL '7 days'` | Returns count |
| 7-day query perf | `EXPLAIN ANALYZE SELECT * FROM silver_indoor_air WHERE timestamp > NOW() - INTERVAL '7 days'` | Execution time < 5 seconds |

### DuckDB Query Execution
```bash
# Connect to DuckDB and run verification queries
docker exec -it duckdb duckdb /data/analysis.duckdb

# Run each test query above
# Record execution times
# Verify schema matches expectations
```

### Grafana Datasource
| Test | Procedure | Expected |
|------|-----------|----------|
| Datasource exists | Navigate to Configuration > Data Sources | DuckDB listed |
| Connection test | Click "Test" button on datasource | Green "Data source is working" message |
| Query editor | Open datasource, try test query | Results returned without error |

### Grafana Dashboards
| Test | Procedure | Expected |
|------|-----------|----------|
| Indoor Air dashboard | Navigate to Dashboards > Indoor Air Quality | Dashboard loads, all panels render |
| Outdoor Weather dashboard | Navigate to Dashboards > Outdoor Weather | Dashboard loads, all panels render |
| Outdoor AQI dashboard | Navigate to Dashboards > Outdoor Air Quality Index | Dashboard loads, all panels render |
| Comparison dashboard | Navigate to Dashboards > Indoor vs Outdoor Comparison | Dashboard loads, all panels render |
| Time range picker | Change to "Last 30 days" | Data updates correctly, no errors |
| Panel queries | Inspect panel queries | Valid SQL syntax, no template variable errors |
| Variables work | Change any dashboard variables | Panels update accordingly |

### Dashboard Content Validation
For each dashboard, verify:
- [ ] At least 4 visualization panels
- [ ] Data points visible (not empty)
- [ ] Axes labeled correctly
- [ ] Legend shows series names
- [ ] Tooltips show values on hover
- [ ] No "No Data" errors
- [ ] No query timeout errors

## 4. Performance Verification

### Query Performance
| Metric | Target | Measurement Method | Pass/Fail |
|--------|--------|-------------------|-----------|
| 7-day query latency | < 5 seconds | `EXPLAIN ANALYZE` in DuckDB | |
| 30-day query latency | < 15 seconds | `EXPLAIN ANALYZE` in DuckDB | |
| 90-day query latency | < 30 seconds | `EXPLAIN ANALYZE` in DuckDB | |
| Dashboard load time | < 3 seconds | Browser dev tools Network tab | |
| Panel refresh time | < 2 seconds | Browser dev tools, manual refresh | |

### Resource Utilization
| Metric | Target | Measurement Method | Pass/Fail |
|--------|--------|-------------------|-----------|
| DuckDB CPU (idle) | < 5% | `docker stats duckdb` | |
| DuckDB CPU (query) | < 50% peak | `docker stats duckdb` during query | |
| DuckDB memory | < 512MB | `docker stats duckdb` | |
| Grafana CPU | < 10% | `docker stats grafana` | |
| Grafana memory | < 256MB | `docker stats grafana` | |

### Data Freshness
- [ ] Query returns data from last hour
- [ ] Timestamp fields are recent (not stale)
- [ ] Real-time dashboard updates work (if auto-refresh enabled)

## 5. Acceptance Criteria Sign-off

### Must Pass (V1 Complete)
- [ ] All containers start successfully and remain healthy for 5 minutes
- [ ] All Silver views (4) are queryable and return valid data
- [ ] All dashboards (4) load without errors
- [ ] 7-day default view returns data in < 5 seconds
- [ ] 30-day extended view returns data in < 15 seconds
- [ ] Dashboard edits persist after container restart
- [ ] Grafana authentication works (default admin/admin)
- [ ] No ERROR messages in logs after 5-minute runtime

### Nice to Have (Stretch Goals)
- [ ] Dashboard refresh rate configurable (5s/10s/30s)
- [ ] Hourly aggregation views created
- [ ] Daily aggregation views created
- [ ] Query performance with HNSW or partitioning optimizations
- [ ] Alert rules configured (future phase)
- [ ] User authentication beyond default admin

### Known Limitations (Acceptable for V1)
- Grafana datasource is read-only (no data modification)
- No authentication beyond default admin user
- No HTTPS/TLS configuration
- Limited to last 90 days of data (storage constraints)

## 6. Rollback Procedure

If verification fails and issues cannot be quickly resolved:

1. **Stop new services**:
   ```bash
   docker-compose stop duckdb grafana
   ```

2. **Remove from docker-compose.yml**:
   - Comment out or remove duckdb service definition
   - Comment out or remove grafana service definition

3. **Restart original stack**:
   ```bash
   docker-compose up -d
   ```

4. **Verify Bronze layer still working**:
   ```bash
   docker-compose logs parquet-writer
   ls -lh /data/bronze/air-quality/
   ```

5. **Document rollback reason** in `product/features/dp-001/completion/ROLLBACK_LOG.md`

## 7. Post-Deployment Monitoring

### First 24 Hours
- [ ] Monitor container health every 4 hours
- [ ] Check log files for errors
- [ ] Verify disk space not rapidly filling
- [ ] Test dashboard access from external device

### First Week
- [ ] Review query performance trends
- [ ] Check for memory leaks (memory usage creep)
- [ ] Validate data completeness (no gaps)
- [ ] User acceptance testing with stakeholders

## 8. Documentation Updates

After successful verification:
- [ ] Update `STATUS.md` to "Completed"
- [ ] Update `deploy/pi/README.md` with Grafana access instructions
- [ ] Create `docs/dashboards/USAGE_GUIDE.md` for end users
- [ ] Add troubleshooting section to deployment docs
- [ ] Update architecture diagrams to include Grafana

## 9. Sign-off

| Role | Name | Date | Status |
|------|------|------|--------|
| Developer | | | |
| Tester | | | |
| Owner | | | |

**Notes**:
- All "Must Pass" criteria must be checked before sign-off
- "Nice to Have" items can be deferred to future iterations
- Any failed criteria must be documented with reason and plan
- Sign-off indicates feature is production-ready

---

**Verification Date**: _______________

**Verified By**: _______________

**Production Deployment Approved**: [ ] Yes [ ] No
