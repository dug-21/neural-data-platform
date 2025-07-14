# Data Ingestion Monitoring Setup

This directory contains the complete monitoring configuration for the data ingestion service, including Grafana dashboards, Prometheus alerts, and PromQL queries.

## Files Overview

- **`grafana-dashboard.json`** - Comprehensive Grafana dashboard for data ingestion monitoring
- **`prometheus-alerts.yml`** - Alert rules for Prometheus Alertmanager
- **`promql-queries.md`** - Reference guide for all PromQL queries used

## Dashboard Features

### 1. Service Overview
- Overall service health status
- Total throughput metrics
- Request distribution by provider
- Overall success/error rates

### 2. Provider Status
- Individual provider health monitoring
- Provider-specific error rates
- Response time comparison
- Detailed health matrix

### 3. Data Flow Pipeline
- Visual pipeline flow representation
- Stage-by-stage throughput metrics
- Pipeline latency analysis
- Bottleneck identification

### 4. Error Analysis
- Error categorization by type
- Provider-specific error distribution
- Time-series error trending
- Root cause analysis support

### 5. Performance Metrics
- Request duration heatmap
- Latency percentile tracking (p50, p90, p95, p99)
- Historical performance trends
- Capacity planning metrics

### 6. Symbol Coverage
- Active symbol count
- Data freshness tracking
- Stale data identification
- Coverage percentage metrics

### 7. Alerts Section
- Active alert display
- Alert history tracking
- Severity-based categorization

## Setup Instructions

### 1. Import Grafana Dashboard

```bash
# Using Grafana UI
1. Navigate to Dashboards → Import
2. Upload grafana-dashboard.json
3. Select your Prometheus datasource
4. Click Import

# Using Grafana API
curl -X POST http://localhost:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d @grafana-dashboard.json
```

### 2. Configure Prometheus Alerts

```bash
# Add to your prometheus.yml
rule_files:
  - '/etc/prometheus/rules/data-ingestion-alerts.yml'

# Copy the alert rules
cp prometheus-alerts.yml /etc/prometheus/rules/data-ingestion-alerts.yml

# Reload Prometheus
curl -X POST http://localhost:9090/-/reload
```

### 3. Configure Alertmanager

Add routing rules to your `alertmanager.yml`:

```yaml
route:
  group_by: ['alertname', 'component', 'severity']
  group_wait: 10s
  group_interval: 5m
  repeat_interval: 12h
  receiver: 'data-ingestion-team'
  routes:
  - match:
      component: data-ingestion
      severity: critical
    receiver: 'data-ingestion-oncall'
    continue: true
  - match:
      component: data-ingestion
      severity: warning
    receiver: 'data-ingestion-team'

receivers:
- name: 'data-ingestion-team'
  email_configs:
  - to: 'data-team@example.com'
  
- name: 'data-ingestion-oncall'
  pagerduty_configs:
  - service_key: 'YOUR_PAGERDUTY_KEY'
```

## Alert Thresholds

### Critical Alerts
- **Service Down**: Service unavailable for >2 minutes
- **Critical Error Rate**: Error rate >10% for >2 minutes
- **No Activity**: Zero throughput for >5 minutes
- **Pipeline Blocked**: Pipeline stage not processing data
- **Connection Pool Exhausted**: No available connections

### Warning Alerts
- **Provider Down**: Individual provider down for >5 minutes
- **High Error Rate**: Error rate >5% for >5 minutes
- **High Latency**: p95 latency >1s for >5 minutes
- **Provider Slow**: Provider p95 latency >2s
- **Low Throughput**: <10 requests/sec for >10 minutes
- **Stale Data**: Symbol not updated for >5 minutes
- **Low Data Freshness**: <90% symbols with fresh data
- **High Resource Usage**: Memory >4GB or CPU >80%

## Dashboard Variables

The dashboard includes the following template variables:

- **`$datasource`**: Prometheus datasource selection
- **`$provider`**: Multi-select for filtering by provider

## Metric Naming Convention

All metrics follow the pattern: `data_ingestion_<component>_<metric>_<unit>`

Examples:
- `data_ingestion_total_requests`
- `data_ingestion_provider_errors_total`
- `data_ingestion_request_duration_bucket`
- `data_ingestion_pipeline_stage_total`

## Performance Optimization

The dashboard includes several recording rules to pre-compute expensive queries:

- `data_ingestion:error_rate:5m` - Pre-computed error rate
- `data_ingestion:latency_p95:5m` - Pre-computed p95 latency
- `data_ingestion:throughput:5m` - Pre-computed throughput
- `data_ingestion:data_freshness_percentage` - Pre-computed freshness

These recording rules improve dashboard performance and reduce Prometheus load.

## Troubleshooting

### Common Issues

1. **Missing Metrics**
   - Verify the data ingestion service is exposing metrics on `/metrics`
   - Check Prometheus targets page for scraping errors
   - Ensure correct job labels in Prometheus configuration

2. **Dashboard Not Loading**
   - Verify Prometheus datasource is configured correctly
   - Check browser console for JavaScript errors
   - Ensure Grafana has proper permissions

3. **Alerts Not Firing**
   - Check Prometheus rules evaluation at `/rules`
   - Verify Alertmanager is receiving alerts at `/alerts`
   - Check alert routing configuration

### Useful Commands

```bash
# Check Prometheus targets
curl http://localhost:9090/api/v1/targets

# Verify metrics are being scraped
curl http://localhost:9090/api/v1/query?query=up{job="data-ingestion"}

# Test alert rules
promtool check rules prometheus-alerts.yml

# Check Alertmanager status
curl http://localhost:9093/api/v1/status
```

## Customization

### Adding New Panels

1. Identify the metrics you want to visualize
2. Write and test the PromQL query
3. Add a new panel to the dashboard
4. Configure visualization options
5. Save and version the dashboard

### Modifying Alert Thresholds

1. Update values in `prometheus-alerts.yml`
2. Test with `promtool check rules`
3. Reload Prometheus configuration
4. Monitor for false positives/negatives

## Best Practices

1. **Regular Review**: Review alert thresholds monthly
2. **Dashboard Versioning**: Version control dashboard JSON
3. **Alert Documentation**: Document runbooks for each alert
4. **Performance Testing**: Load test to validate thresholds
5. **Backup**: Regular backup of dashboard and rules

## Support

For issues or questions:
1. Check the PromQL queries reference
2. Review Prometheus/Grafana logs
3. Contact the data ingestion team