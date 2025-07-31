# Fix: Startup Issues (Duplicate Metrics & Config)

## Issues Fixed

### Issue 1: Duplicate `data_ingestion_health_status`
The metric was defined twice in `utils/metrics.py`:
1. Line 127: With a 'component' label (as `self.health_check_status`)
2. Line 295: Without any labels

### Issue 2: Duplicate `data_ingestion_data_flow_age_seconds`
The metric was defined twice in `utils/metrics.py`:
1. Line 172: As `self.data_flow_age`
2. Line 301: As `self.data_flow_age_seconds`

## Root Cause
Multiple developers added metrics in different phases without checking for existing definitions, causing duplicate metric registrations in Prometheus.

## Solutions Applied

### For `data_ingestion_health_status`:
1. Removed the duplicate definition at line 295 (the one without labels)
2. Updated `health_check.py` line 423 to use the metric with the 'component' label:
   ```python
   metrics.health_check_status.labels(component='overall').set(1 if is_healthy else 0)
   ```

### For `data_ingestion_data_flow_age_seconds`:
1. Removed the duplicate definition at line 301 (`self.data_flow_age_seconds`)
2. Updated `health_check.py` line 434 to use the correct attribute name:
   ```python
   metrics.data_flow_age.labels(provider=provider, symbol=symbol).set(age_seconds)
   ```

## Files Modified
- `/workspaces/neural-trader/data_ingestion/utils/metrics.py` - Removed duplicate metric definitions
- `/workspaces/neural-trader/data_ingestion/utils/health_check.py` - Updated to use correct metric names

## Testing
After rebuilding the container, the service should start without duplicate metric errors.

```bash
# Rebuild and restart
docker-compose build data-ingestion
docker-compose up -d data-ingestion
docker-compose logs -f data-ingestion
```

### Issue 3: Wrong CircuitBreakerConfig Parameter
The circuit breaker configuration was using `recovery_timeout` instead of the correct parameter `timeout`.

## Solutions Applied

### For CircuitBreakerConfig:
Changed all occurrences of `recovery_timeout` to `timeout` in main.py:
```python
CircuitBreakerConfig(
    failure_threshold=5,
    timeout=60.0,  # was recovery_timeout
    success_threshold=2
)
```

## Prevention
To prevent future issues:
1. Always search for existing metric names before adding new ones
2. Use consistent naming for metric attributes
3. Check class/function signatures before using parameters
4. Consider adding unit tests to catch configuration errors