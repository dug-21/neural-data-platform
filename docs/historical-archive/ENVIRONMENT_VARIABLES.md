# Data Ingestion Environment Variables

## Provider Selection

The data ingestion service looks for provider configuration in the following order:

1. **Command line arguments** (highest priority)
   - `--providers polygon` when starting the service

2. **PRIMARY_PROVIDER** environment variable
   - Set in docker-compose: `PRIMARY_PROVIDER=polygon`
   - This is what's used in production docker-compose.prod.yml

3. **DEFAULT_PROVIDER** environment variable
   - Alternative to PRIMARY_PROVIDER
   - Set: `DEFAULT_PROVIDER=polygon`

4. **ACTIVE_PROVIDERS** environment variable (for multiple providers)
   - Comma-separated list: `ACTIVE_PROVIDERS=["polygon","alpaca"]`

5. **Hardcoded default** (lowest priority)
   - Falls back to `alpaca` if nothing else is configured

## Required Environment Variables for Polygon

```bash
# Provider selection
PRIMARY_PROVIDER=polygon

# Polygon API credentials
POLYGON_API_KEY=your-polygon-api-key

# Optional Polygon settings
POLYGON_USE_DELAYED=false        # Use real-time feed (false) or delayed feed (true)
POLYGON_WEBSOCKET_ENABLED=true   # Enable WebSocket streaming

# Symbols to track
SYMBOLS=AAPL,MSFT,GOOGL,AMZN    # Comma-separated list
```

## Docker Production Setup

In `docker/production/.env`, you should have:

```bash
# Set polygon as primary provider
PRIMARY_PROVIDER=polygon

# Polygon API key
POLYGON_API_KEY=your-actual-api-key

# Disable Alpaca WebSocket since we're using Polygon
ALPACA_WS_ENABLED=false
```

## Verification

After rebuilding and restarting, you should see in the logs:
```
Using primary provider: polygon
```

If you still see it using alpaca, check:
1. The .env file is in the correct location: `docker/production/.env`
2. The environment variable is being passed through docker-compose
3. The container was rebuilt after code changes: `docker-compose build data-ingestion`

## Startup Script

The startup script (`start-data-ingestion.sh`) now checks for:
- `PRIMARY_PROVIDER` - Used if set
- `DEFAULT_PROVIDER` - Used as fallback
- Passes the provider to the Python command line

This ensures the provider selection works correctly in production containers.