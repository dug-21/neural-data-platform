# Data Provider Strategy

## Current Setup (Optimized for Cost & Performance)

### Real-Time Data: Alpaca (Primary)
- **WebSocket streaming** for real-time market data
- **Free tier** includes real-time quotes
- **Reliable** for live trading decisions
- **Low latency** for immediate updates

### Historical Data: Polygon (Secondary)
- **Polygon Basic Plan** - 15-minute delayed data only
- **Best for**: Historical backfill and analysis
- **Limitations**: 
  - No real-time data
  - No minute aggregates (403 errors)
  - 15-minute delay on quotes
- **Use cases**:
  - Daily/weekly/monthly historical data
  - EOD (End of Day) analysis
  - Long-term trend analysis

## Configuration

```bash
# In docker/production/.env
PRIMARY_PROVIDER="alpaca"          # Real-time data
FALLBACK_PROVIDERS=["polygon"]     # Historical backfill
ACTIVE_PROVIDERS=["alpaca","polygon"]  # Both available

# Polygon-specific settings
POLYGON_BASIC_PLAN=true            # Limits to EOD data
POLYGON_USE_DELAYED=true           # 15-min delayed quotes
```

## Data Flow

1. **Real-Time Streaming** (Market Hours)
   - Alpaca WebSocket → Live quotes/trades
   - Low latency for trading decisions
   - Continuous updates during market hours

2. **Historical Backfill** (Off Hours)
   - Polygon API → Daily bars for history
   - Scheduled batch jobs for EOD data
   - Building historical datasets

3. **Failover Logic**
   - Primary: Alpaca WebSocket
   - If Alpaca fails → Alpaca HTTP polling
   - If still failing → Polygon (delayed data warning)

## Implementation Notes

### Batch Scheduler Configuration
```python
# Use Polygon for daily historical data
scheduler.schedule_job(
    'daily_historical',
    '0 6 * * *',  # 6 AM daily
    {
        'providers': ['polygon'],  # Specifically use Polygon
        'interval': '1day',        # Daily bars work on Basic plan
        'lookback_days': 30
    }
)

# Use Alpaca for intraday data
scheduler.schedule_job(
    'intraday_update',
    '*/5 * * * *',  # Every 5 minutes
    {
        'providers': ['alpaca'],   # Real-time provider
        'interval': '5min',
        'lookback_minutes': 60
    }
)
```

### Cost Optimization
- **Alpaca**: Free tier sufficient for real-time
- **Polygon Basic**: $29/month for historical EOD data
- **Combined**: Best of both worlds at minimal cost

### Future Upgrade Path
When ready to upgrade Polygon:
1. **Starter Plan** ($99/month): Real-time data, minute aggregates
2. **Developer Plan** ($199/month): Full historical, all endpoints
3. Keep Alpaca as backup for redundancy

## Monitoring

The system will log:
- Which provider is active for each data type
- Failover events
- Data quality metrics (delays, gaps)
- API usage statistics

This hybrid approach maximizes data availability while minimizing costs.