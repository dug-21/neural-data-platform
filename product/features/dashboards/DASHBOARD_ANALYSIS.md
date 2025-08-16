# Grafana Dashboard Configuration Analysis

## Overview

Analysis of four Grafana dashboard configurations in the neural-trader project, focusing on metric expectations, query patterns, and potential naming mismatches.

## Dashboard Summary

| Dashboard | File | Purpose | Primary Datasources |
|-----------|------|---------|-------------------|
| Data Ingestion Monitoring | `data-ingestion.json` | Monitor data ingestion pipeline health | Prometheus |
| Market Data - TimescaleDB Direct | `market-data-timescale.json` | Real-time market data visualization | TimescaleDB |
| Neural Trader Complete Dashboard | `neural-trader-complete.json` | Comprehensive system monitoring | TimescaleDB |
| Neural Trader Overview | `neural-trader-overview.json` | High-level system status | Prometheus |

## Expected Metric Names by Dashboard

### 1. Data Ingestion Dashboard (Prometheus Metrics)

**Service Health & Status:**
- `up{job="data-ingestion"}`
- `up{job="data-ingestion-provider"}` (by provider)

**Request Metrics:**
- `data_ingestion_provider_requests_total` (by provider)
- `data_ingestion_total_requests`
- `data_ingestion_errors_total`
- `data_ingestion_provider_errors_total` (by provider)
- `data_ingestion_errors_by_type_total` (by error_type)

**Performance Metrics:**
- `data_ingestion_provider_request_duration_bucket` (histogram, by provider, le)
- `data_ingestion_request_duration_bucket` (histogram, by le)

**Pipeline Metrics:**
- `data_ingestion_fetch_operations_total`
- `data_ingestion_pipeline_stage_total` (by stage)
- `data_ingestion_pipeline_stage_duration_bucket` (histogram, by stage, le)

**Symbol Coverage:**
- `data_ingestion_active_symbols`
- `data_ingestion_last_update_timestamp` (by symbol)
- `data_ingestion_symbol_data_fresh{fresh="true"}`

**Alert Integration:**
- `ALERTS{alertname=~".*DataIngestion.*", alertstate="firing"}`

### 2. Market Data TimescaleDB Dashboard (SQL Queries)

**Market Data Table:**
```sql
SELECT time, symbol, close FROM market_data WHERE $__timeFilter(time)
SELECT symbol, close, volume, time FROM market_data WHERE time IN (SELECT MAX(time) FROM market_data GROUP BY symbol)
SELECT COUNT(*) FROM market_data
SELECT symbol, COUNT(*) FROM market_data GROUP BY symbol
SELECT DISTINCT ON (symbol) symbol, close, metadata->>'52_week_high', metadata->>'52_week_low' FROM market_data ORDER BY symbol, time DESC
```

**Expected Table Structure:**
- `market_data` table with columns: `time`, `symbol`, `close`, `open`, `volume`, `metadata` (jsonb)
- Time column must support `$__timeFilter()` macro
- Metadata field expected to contain `52_week_high` and `52_week_low` keys

### 3. Neural Trader Complete Dashboard (TimescaleDB)

**Market Data:**
- Same `market_data` table structure as above
- Additional query patterns for percentage calculations

**Trading Decisions:**
```sql
SELECT time, symbol, action, confidence, position_size, agent_id FROM trading_decisions
SELECT time_bucket('15 minute', time), action, COUNT(*) FROM trading_decisions GROUP BY 1, action
SELECT action, COUNT(*) FROM trading_decisions WHERE time > NOW() - INTERVAL '24 hours' GROUP BY action
SELECT AVG(confidence) FROM trading_decisions WHERE time > NOW() - INTERVAL '1 hour'
```

**Performance Metrics:**
```sql
SELECT time, metric_name, value FROM performance_metrics 
WHERE metric_name IN ('latency', 'throughput', 'cpu_usage', 'memory_usage')
```

**Expected Table Structures:**
- `trading_decisions`: `time`, `symbol`, `action`, `confidence`, `position_size`, `agent_id`
- `performance_metrics`: `time`, `metric_name`, `value`

### 4. Neural Trader Overview Dashboard (Prometheus)

**Service Status:**
- `up{job="neural-trader"}`

**Trading Metrics:**
- `trades_executed_total`
- `total_pnl`

**Performance Metrics:**
- `http_request_duration_seconds_bucket` (histogram, by le)
- `market_data_received_bytes_total`

## Datasource Configurations

### Variable Definitions

**Data Ingestion Dashboard:**
- `${datasource}` - Prometheus datasource variable
- `${provider}` - Multi-select provider filter from `label_values(data_ingestion_provider_requests_total, provider)`

**Neural Trader Complete Dashboard:**
- `${DS_TIMESCALEDB}` - TimescaleDB datasource variable (hidden)

### Hardcoded Assumptions

**Job Names:**
- `data-ingestion` (main service)
- `data-ingestion-provider` (provider-specific)
- `neural-trader` (main trading service)

**Time Intervals:**
- Rate calculations: `[5m]` (standard)
- Provider request duration: `[5m]`
- Pipeline stage metrics: `[5m]`
- Alert lookback: Various intervals

**Label Expectations:**
- `provider` label on ingestion metrics
- `stage` label on pipeline metrics
- `error_type` label on error metrics
- `symbol` label on market data
- `action` label on trading decisions

## Query Patterns Analysis

### Prometheus Patterns

1. **Rate Calculations:**
   ```promql
   rate(metric_name[5m])
   sum(rate(metric_name[5m])) by (label)
   ```

2. **Error Rate Calculations:**
   ```promql
   (1 - (sum by (provider) (rate(errors_total[5m])) / sum by (provider) (rate(requests_total[5m])))) * 100
   ```

3. **Histogram Quantiles:**
   ```promql
   histogram_quantile(0.95, sum by (provider, le) (rate(duration_bucket[5m])))
   ```

4. **Availability:**
   ```promql
   up{job="service-name"}
   ```

### TimescaleDB Patterns

1. **Time Bucketing:**
   ```sql
   time_bucket('5 minute', time)
   time_bucket('15 minute', time)
   time_bucket('1 minute', time)
   ```

2. **Latest Values per Group:**
   ```sql
   SELECT DISTINCT ON (symbol) ... ORDER BY symbol, time DESC
   ```

3. **JSON Field Access:**
   ```sql
   metadata->>'52_week_high'
   metadata->>'52_week_low'
   ```

4. **Time Filtering:**
   ```sql
   WHERE $__timeFilter(time)
   WHERE time > NOW() - INTERVAL '1 hour'
   ```

## Potential Issues and Mismatches

### 1. Service Discovery Consistency

**Issue:** Job names must match between Prometheus scraping config and dashboard queries
- `job="data-ingestion"` vs `job="data-ingestion-service"`
- `job="neural-trader"` vs `job="neural-trader-api"`

### 2. Metric Naming Inconsistencies

**Potential Issues:**
- Data ingestion metrics use `data_ingestion_` prefix consistently
- Neural trader metrics mix patterns: `trades_executed_total` vs `total_pnl`
- Some metrics lack namespace prefixes

### 3. Label Standardization

**Missing Label Consistency:**
- Provider labels may not match across all metrics
- Stage names in pipeline metrics need standardization
- Error type classifications require predefined set

### 4. Database Schema Dependencies

**TimescaleDB Requirements:**
- `market_data.metadata` must be JSONB type
- Time columns must support TimescaleDB time functions
- Required indexes for performance on time-based queries

### 5. Variable Configuration Dependencies

**Data Ingestion Dashboard:**
- Requires `label_values(data_ingestion_provider_requests_total, provider)` to populate provider filter
- If metric doesn't exist, variable population fails

### 6. Alert Integration

**Dependencies:**
- Alert manager must be configured with `DataIngestion` prefix
- Alert rules must set `alertstate="firing"` label
- Alert severity and description fields expected

## Recommendations

### 1. Metric Naming Standards

Establish consistent prefixes:
- `neural_trader_*` for main application metrics  
- `data_ingestion_*` for ingestion pipeline metrics
- `market_data_*` for market-specific metrics

### 2. Label Standardization

Define standard label sets:
- `job`, `instance` (standard Prometheus)
- `symbol` for market instruments
- `provider` for data providers
- `stage` for pipeline stages
- `action` for trading decisions

### 3. Service Discovery

Ensure job names in Prometheus config match dashboard expectations:
```yaml
# prometheus.yml
- job_name: 'neural-trader'
- job_name: 'data-ingestion'
```

### 4. Database Schema Validation

Verify TimescaleDB table structures:
```sql
-- Validate required columns exist
SELECT column_name, data_type FROM information_schema.columns 
WHERE table_name IN ('market_data', 'trading_decisions', 'performance_metrics');
```

### 5. Dashboard Variable Testing

Test variable queries independently:
```promql
label_values(data_ingestion_provider_requests_total, provider)
```

### 6. Monitoring Coverage Gaps

Consider adding dashboards for:
- Infrastructure metrics (CPU, memory, disk)
- Database performance metrics
- Network and connectivity metrics
- Neural model performance metrics

## Implementation Priority

1. **High:** Verify service job names match
2. **High:** Validate database table schemas  
3. **Medium:** Standardize metric naming conventions
4. **Medium:** Test dashboard variable population
5. **Low:** Add missing monitoring coverage