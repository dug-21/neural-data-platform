# Feature Gap Matrix: air-001 Specification vs Implementation

**Analysis Date:** December 14, 2025
**Specification Version:** 1.2.0
**Implementation Assessment:** Post-Phase 1 TDD

---

## Summary

| Category | Requirements | Implemented | Gap | E2E Blocking |
|----------|--------------|-------------|-----|--------------|
| FR-1: Data Ingestion | 5 | 2 | 60% | YES |
| FR-2: Storage | 4 | 3 | 25% | NO |
| FR-3: Querying | 4 | 3 | 25% | NO |
| FR-4: Forecasting | 4 | 0 | 100% | YES |
| FR-5: Alerting | 4 | 1 | 75% | YES |
| FR-6: MCP Integration | 5 | 0 | 100% | NO |
| FR-7: Domain Extensibility | 4 | 2 | 50% | NO |
| FR-8: Configuration | 7 | 3 | 57% | NO |

**Overall:** ~45-50% of functional requirements implemented

---

## FR-1: Data Ingestion

| Requirement | Status | Completeness | Blocking |
|-------------|--------|--------------|----------|
| **FR-1.1** MQTT Client Connection | NOT IMPLEMENTED | 0% | YES |
| **FR-1.2** Message Parsing & Validation | Domain layer complete | 90% | NO |
| **FR-1.3** Data Quality Assessment | Basic validation only | 40% | NO |
| **FR-1.4** Ingestion Rate Limits | NOT IMPLEMENTED | 0% | NO |
| **FR-1.5** Config Endpoint Retrieval | NOT IMPLEMENTED | 0% | NO |

### Details

**FR-1.1: MQTT Client Connection**
- Missing: rumqttc client initialization
- Missing: Topic subscription (`airgradient/readings/{SERIAL}`)
- Missing: Auto-reconnect with exponential backoff
- Missing: Connection event logging

**FR-1.2: Message Parsing**
- COMPLETE: `parse_mqtt_payload()` handles all 29 fields
- COMPLETE: `parse_local_api_payload()` reuses MQTT parser
- COMPLETE: Type conversion (JSON to Rust types)
- MISSING: DLQ integration for malformed messages

**FR-1.3: Data Quality Assessment**
- PARTIAL: Range validation implemented
- MISSING: Quality scoring (completeness × calibration × freshness)
- MISSING: Quality flags (`co2_warmup_period`, `pm_high_humidity`)

---

## FR-2: Storage

| Requirement | Status | Completeness | Blocking |
|-------------|--------|--------------|----------|
| **FR-2.1** Parquet File Format | Core storage complete | 85% | NO |
| **FR-2.2** Daily Partitioning | Core storage complete | 95% | NO |
| **FR-2.3** Write-Ahead Log | Implemented | 90% | NO |
| **FR-2.4** Storage Capacity Mgmt | NOT IMPLEMENTED | 0% | NO |

### Details

**FR-2.1: Parquet File Format**
- COMPLETE: Columnar storage with Snappy compression
- PARTIAL: Schema missing `quality_score`, `quality_flags` fields
- PARTIAL: Tags not stored in Parquet (only timestamp, location_id, value)

**FR-2.2: Daily Partitioning**
- COMPLETE: `data/{location_id}/year={YYYY}/month={MM}/day={DD}/readings.parquet`
- COMPLETE: Atomic commits (temp file → rename)

**FR-2.3: Write-Ahead Log**
- COMPLETE: Append-only WAL for crash recovery
- COMPLETE: Replay on startup
- COMPLETE: Delete after successful commit

**FR-2.4: Storage Capacity Management**
- MISSING: Retention policy configuration
- MISSING: Auto-delete old partitions
- MISSING: Storage metrics

---

## FR-3: Querying

| Requirement | Status | Completeness | Blocking |
|-------------|--------|--------------|----------|
| **FR-3.1** Time-Range Queries | API + core complete | 85% | NO |
| **FR-3.2** Aggregations | API + core complete | 90% | NO |
| **FR-3.3** Multi-Location Queries | NOT IMPLEMENTED | 0% | NO |
| **FR-3.4** In-Memory Caching | NOT IMPLEMENTED | 0% | NO |

### Details

**FR-3.1: Time-Range Queries**
- COMPLETE: `GET /api/v1/readings?start=&end=`
- COMPLETE: Polars predicate pushdown
- MISSING: <100ms performance validation

**FR-3.2: Aggregations**
- COMPLETE: Mean, min, max, p50, p95 supported
- COMPLETE: 1min, 5min, 1hour, 1day intervals
- COMPLETE: `groupby_dynamic()` for resampling

---

## FR-4: Forecasting (CRITICAL GAP)

| Requirement | Status | Completeness | Blocking |
|-------------|--------|--------------|----------|
| **FR-4.1** Model Integration (ruv-FANN) | NOT INTEGRATED | 0% | YES |
| **FR-4.2** Feature Engineering | NOT IMPLEMENTED | 0% | YES |
| **FR-4.3** Forecast Storage | NOT IMPLEMENTED | 0% | NO |
| **FR-4.4** Model Retraining | DEFERRED | N/A | NO |

### Details

**FR-4.1: Model Integration**
- ruv-FANN library exists in `vendor/ruv-fann/neuro-divergent/`
- 27+ models available (LSTM, NBEATS, NHITS, TFT, etc.)
- NOT connected to air quality app
- Forecast endpoint returns empty predictions

**FR-4.2: Feature Engineering**
- MISSING: Time features (hour_of_day, day_of_week)
- MISSING: Lag features (pm25_lag_1h, pm25_lag_24h)
- MISSING: Rolling statistics (rolling_mean, rolling_std)
- MISSING: Z-score normalization

---

## FR-5: Alerting (CRITICAL GAP)

| Requirement | Status | Completeness | Blocking |
|-------------|--------|--------------|----------|
| **FR-5.1** Health Threshold Alerts | Stub example only | 10% | YES |
| **FR-5.2** Predictive Alerts | NOT IMPLEMENTED | 0% | YES |
| **FR-5.3** Alert Delivery | In-memory only | 25% | NO |
| **FR-5.4** Alert History | In-memory only | 30% | NO |

### Details

**FR-5.1: Health Threshold Alerts**
- MISSING: CO2 thresholds (>1000, >1500, >2000 ppm)
- MISSING: PM2.5 thresholds (>12, >35, >55 µg/m³)
- MISSING: VOC thresholds (>150, >200, >300 index)
- MISSING: Alert deduplication (10% drop before clearing)
- PARTIAL: Example alert engine in test file (not integrated)

**FR-5.2: Predictive Alerts**
- Depends on FR-4 forecasting
- MISSING: Alert if p90 forecast exceeds threshold
- MISSING: Lead time configuration

---

## FR-6: MCP Integration (NOT STARTED)

| Requirement | Status | Completeness | Blocking |
|-------------|--------|--------------|----------|
| **FR-6.1** Air Quality Query Tool | NOT IMPLEMENTED | 0% | NO |
| **FR-6.2** Forecast Tool | NOT IMPLEMENTED | 0% | NO |
| **FR-6.3** Alert Retrieval Tool | NOT IMPLEMENTED | 0% | NO |
| **FR-6.4** Sensor Health Tool | NOT IMPLEMENTED | 0% | NO |
| **FR-6.5** Recommendation Tool | NOT IMPLEMENTED | 0% | NO |

### Details

All 5 MCP tools specified in FR-6 are not implemented. The `mcp-trading-server` provides patterns that can be adapted:
- `MarketDataTool` pattern → `air_quality_query`
- `NeuralPredictionTool` pattern → `air_quality_forecast`
- `HealthMonitorTool` pattern → `air_quality_alerts`, `air_quality_sensor_health`

Estimated effort: 2-3 days for all 5 tools

---

## FR-7: Domain Extensibility

| Requirement | Status | Completeness | Blocking |
|-------------|--------|--------------|----------|
| **FR-7.1** Generic Core Traits | Partial | 50% | NO |
| **FR-7.2** Air Quality Adapter | Complete | 85% | NO |
| **FR-7.3** New Domain Template | NOT IMPLEMENTED | 0% | NO |
| **FR-7.4** Domain Registry | NOT IMPLEMENTED | 0% | NO |

### Details

**FR-7.1: Generic Core Traits**
- COMPLETE: `TimeSeriesPoint` in core
- PARTIAL: `DomainAdapter` trait not formalized
- MISSING: Domain-agnostic health thresholds

**FR-7.2: Air Quality Adapter**
- COMPLETE: Parser, validator, adapter
- MISSING: Health threshold enums

---

## FR-8: Configuration Management

| Requirement | Status | Completeness | Blocking |
|-------------|--------|--------------|----------|
| **FR-8.1** config-store Integration | NOT INTEGRATED | 0% | NO |
| **FR-8.2** YAML Configuration Files | Partial | 50% | NO |
| **FR-8.3** GitHub-Sourced Config | NOT IMPLEMENTED | 0% | NO |
| **FR-8.4** Air Quality Config Schema | Partial | 40% | NO |
| **FR-8.5** Configuration Hot-Reload | NOT IMPLEMENTED | 0% | NO |
| **FR-8.6** Configuration Validation | Partial | 30% | NO |
| **FR-8.7** Dynamic Unit Handling | NOT IMPLEMENTED | 0% | NO |

### Details

**FR-8.2: YAML Configuration**
- COMPLETE: Basic YAML loading
- MISSING: Alert thresholds in config
- MISSING: Retention policy in config
- MISSING: Forecast settings in config

---

## E2E Blocking Issues Summary

### Must Fix for E2E Testing

1. **FR-1.1: MQTT Client** - No data can enter the system
2. **FR-4.1: Forecasting** - Core feature not functional
3. **FR-5.1: Alerting** - Core feature not functional

### High Priority but Not Blocking

4. FR-1.3: Quality scoring (data quality visible but not critical)
5. FR-2.4: Storage management (can run without retention)
6. FR-6.x: MCP tools (Claude integration is additive)

### Deferred

7. FR-3.3: Multi-location (single sensor v1.0)
8. FR-3.4: Caching (performance optimization)
9. FR-4.4: Retraining (use pre-trained models)
10. FR-7.3, FR-7.4: Domain extensibility (v1.1+)
11. FR-8.3, FR-8.5: Advanced config (v1.1+)

---

## Effort Estimates

| Gap Area | Est. Hours | Priority |
|----------|------------|----------|
| MQTT Ingestion | 40-60 | CRITICAL |
| Forecasting Integration | 50-70 | CRITICAL |
| Alert Generation | 30-40 | CRITICAL |
| MCP Tools | 20-30 | HIGH |
| Quality Scoring | 15-20 | MEDIUM |
| Storage Retention | 10-15 | MEDIUM |
| Config Enhancements | 20-30 | LOW |
| Multi-Location | 15-20 | LOW |

**Total to E2E Ready:** ~200-280 hours (5-7 developer weeks)
