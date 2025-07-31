# Weekend Health Check Fix

## Problem
The health check was reporting WebSocket connections as unhealthy on weekends when stock markets are closed. This was causing false alerts since no data is expected during market closure.

## Root Cause
The health check logic was too simplistic:
- WebSocket health was determined solely by `active_connections > 0`
- Data flow health required active streams regardless of market hours
- No consideration for weekends, holidays, or after-hours periods

## Solution
Implemented market-aware health checking:

### 1. Created Market Hours Utility (`utils/market_hours.py`)
- Checks market status for different exchanges (US stocks, crypto, forex)
- Handles weekends, holidays, and trading hours
- Provides `is_market_data_expected()` function for each provider

### 2. Updated Health Check Logic
- WebSocket health now considers market hours:
  - If markets are closed, no connections is considered healthy
  - Circuit breaker doesn't penalize for closed markets
- Data flow health is more lenient:
  - Allows 24-hour staleness for closed markets
  - No flows required when all markets are closed
  - Startup grace period when no flows exist yet

### 3. Key Changes
```python
# WebSocket Health
if not any_market_open:
    # Markets closed - no connections expected
    ws_status['healthy'] = True
    ws_status['market_status'] = {
        'all_markets_closed': True,
        'message': 'All markets are closed - no data expected'
    }

# Data Flow Health  
if not any_market_open:
    # Don't require active flows when markets closed
    flow_status['healthy'] = True
    flow_status['message'] = 'Markets closed - data staleness is expected'
```

## Benefits
- No false alerts on weekends/holidays
- Accurate health status based on market conditions
- Circuit breakers don't trip unnecessarily
- Better operational monitoring

## Testing
Run `test_weekend_health.py` to verify the fix:
```bash
python test_weekend_health.py
```

Expected output shows:
- WebSocket: Healthy (markets closed)
- Data Flow: Healthy (no flows expected)
- Market status for each provider
- Next market open time