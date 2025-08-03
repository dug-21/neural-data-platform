# Docker Environment Variables Documentation

This document lists all environment variables required for running Neural Trader with Docker Compose.

## Required Environment Variables (Secrets)

These **MUST** be set as environment variables. They will NOT be loaded from .env files for security reasons.

### Database Passwords
- `POSTGRES_PASSWORD` - PostgreSQL/TimescaleDB password (REQUIRED)
- `REDIS_PASSWORD` - Redis password (REQUIRED)

### API Keys
- `IEX_CLOUD_API_KEY` - IEX Cloud API key for market data
- `ALPHA_VANTAGE_API_KEY` - Alpha Vantage API key for market data
- `POLYGON_API_KEY` - Polygon.io API key for market data
- `FINNHUB_API_KEY` - Finnhub API key for market data
- `FRED_API_KEY` - Federal Reserve Economic Data API key
- `REDDIT_CLIENT_ID` - Reddit API client ID
- `REDDIT_CLIENT_SECRET` - Reddit API client secret
- `QUANDL_API_KEY` - Quandl API key for financial data
- `NEWSAPI_KEY` - NewsAPI key for news data
- `YAHOO_API_KEY` - Yahoo Finance API key (if applicable)
- `NASDAQ_API_KEY` - NASDAQ Data Link API key

### Admin Passwords
- `GRAFANA_ADMIN_PASSWORD` - Grafana admin password (REQUIRED for production)
- `PGADMIN_DEFAULT_PASSWORD` - pgAdmin password (REQUIRED for development profile)

## Optional Configuration Variables

These can be set via environment variables OR in .env files.

### Database Configuration
- `POSTGRES_USER` - PostgreSQL username (default: `neural_trader`)
- `POSTGRES_DB` - PostgreSQL database name (default: `neural_trader_db`)
- `TIMESCALE_PASSWORD` - Alias for POSTGRES_PASSWORD in data_ingestion service

### Application Configuration
- `LOG_LEVEL` - Logging level (default: `INFO`)
- `RUST_LOG` - Rust logging level (default: `info`)
- `RUST_BACKTRACE` - Rust backtrace setting (default: `1`)

### Rate Limit Overrides
- `RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_MINUTE` - Override Alpha Vantage rate limit
- `RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_DAY` - Override Alpha Vantage daily limit
- `RATE_LIMIT_POLYGON_CALLS_PER_MINUTE` - Override Polygon rate limit
- `RATE_LIMIT_FINNHUB_CALLS_PER_MINUTE` - Override Finnhub rate limit
- `RATE_LIMIT_NEWSAPI_CALLS_PER_DAY` - Override NewsAPI daily limit
- `RATE_LIMIT_FRED_CALLS_PER_MINUTE` - Override FRED rate limit
- `RATE_LIMIT_REDDIT_CALLS_PER_MINUTE` - Override Reddit rate limit
- `RATE_LIMIT_NASDAQ_CALLS_PER_DAY` - Override NASDAQ daily limit
- `RATE_LIMIT_YAHOO_CALLS_PER_DAY` - Override Yahoo daily limit

### Admin Configuration
- `GRAFANA_ADMIN_USER` - Grafana admin username (default: `admin`)
- `PGADMIN_DEFAULT_EMAIL` - pgAdmin email (default: `admin@neuraltrader.local`)

## Setting Environment Variables

### IMPORTANT: No Secrets on Disk Policy
**Never write secrets to files**. All secrets must be set as environment variables in memory only.

### Using the setup script (Recommended)
```bash
# Source the script to generate passwords in memory
source ./scripts/setup-docker-env.sh

# Set your API keys
export IEX_CLOUD_API_KEY="your-actual-key"
export ALPHA_VANTAGE_API_KEY="your-actual-key"
# ... set other API keys ...

# Verify all variables are set
./scripts/check-env.sh

# Start services
docker-compose up -d
```

### Manual setup (Linux/Mac)
```bash
# Generate and export passwords
export POSTGRES_PASSWORD="$(openssl rand -base64 32)"
export REDIS_PASSWORD="$(openssl rand -base64 32)"
export GRAFANA_ADMIN_PASSWORD="$(openssl rand -base64 32)"

# Set API keys
export ALPHA_VANTAGE_API_KEY="your-api-key"
export FINNHUB_API_KEY="your-api-key"
# ... set other API keys ...

# Optional configurations (these CAN go in .env file)
export LOG_LEVEL="DEBUG"
export RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_MINUTE="10"
```

### Using .env file (for non-secrets ONLY)
Create a `.env` file with non-secret configurations:
```bash
# Non-secret configurations only
# DO NOT PUT PASSWORDS OR API KEYS HERE
LOG_LEVEL=DEBUG
POSTGRES_USER=neural_trader
POSTGRES_DB=neural_trader_db
RUST_LOG=info
```

### One-liner docker-compose run
```bash
POSTGRES_PASSWORD="$(openssl rand -base64 32)" \
REDIS_PASSWORD="$(openssl rand -base64 32)" \
GRAFANA_ADMIN_PASSWORD="$(openssl rand -base64 32)" \
ALPHA_VANTAGE_API_KEY="your-key" \
docker-compose up -d
```

### Using a secure password manager
```bash
# Example with 1Password CLI
export POSTGRES_PASSWORD="$(op read "op://vault/neural-trader/postgres-password")"
export REDIS_PASSWORD="$(op read "op://vault/neural-trader/redis-password")"
export ALPHA_VANTAGE_API_KEY="$(op read "op://vault/neural-trader/alpha-vantage-key")"

# Example with HashiCorp Vault
export POSTGRES_PASSWORD="$(vault kv get -field=password secret/neural-trader/postgres)"
export REDIS_PASSWORD="$(vault kv get -field=password secret/neural-trader/redis)"
```

### Using Docker Secrets (Swarm mode)
For production deployments using Docker Swarm:
```bash
echo "secure-password" | docker secret create postgres_password -
echo "redis-password" | docker secret create redis_password -
```

## Example: Complete Environment Setup

```bash
#!/bin/bash
# setup-env.sh - Set up environment for Neural Trader

# Required secrets (must be environment variables)
export POSTGRES_PASSWORD="$(openssl rand -base64 32)"
export REDIS_PASSWORD="$(openssl rand -base64 32)"
export GRAFANA_ADMIN_PASSWORD="$(openssl rand -base64 24)"

# API Keys (get from respective providers)
export IEX_CLOUD_API_KEY="your-iex-key"
export ALPHA_VANTAGE_API_KEY="your-alpha-vantage-key"
export POLYGON_API_KEY="your-polygon-key"
export FINNHUB_API_KEY="your-finnhub-key"
export FRED_API_KEY="your-fred-key"
export REDDIT_CLIENT_ID="your-reddit-client-id"
export REDDIT_CLIENT_SECRET="your-reddit-secret"
export NEWSAPI_KEY="your-newsapi-key"

# Optional configurations (can be in .env)
export LOG_LEVEL="INFO"
export POSTGRES_USER="neural_trader"
export POSTGRES_DB="neural_trader_db"

# Rate limit overrides (optional)
export RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_MINUTE="10"
export RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_DAY="1000"

# Start services
docker-compose up -d
```

## Security Best Practices

1. **Never commit secrets** to version control
2. **Use strong passwords** - Generate with `openssl rand -base64 32`
3. **Rotate credentials** regularly
4. **Use secret management** tools in production:
   - HashiCorp Vault
   - AWS Secrets Manager
   - Azure Key Vault
   - Kubernetes Secrets

5. **Restrict file permissions** on any files containing secrets:
   ```bash
   chmod 600 secrets.env
   ```

## Verifying Environment Variables

To verify all required variables are set:
```bash
# Check if required variables are set
for var in POSTGRES_PASSWORD REDIS_PASSWORD GRAFANA_ADMIN_PASSWORD; do
  if [ -z "${!var}" ]; then
    echo "ERROR: $var is not set"
  else
    echo "✓ $var is set"
  fi
done

# List all Neural Trader related environment variables
env | grep -E "(POSTGRES|REDIS|API_KEY|RATE_LIMIT|GRAFANA|PGADMIN)" | sort
```

## Troubleshooting

### Service won't start
- Check logs: `docker-compose logs service-name`
- Verify required environment variables are set
- Ensure passwords don't contain special characters that need escaping

### Connection refused
- Verify service names in connection strings match docker-compose service names
- Check that passwords in connection strings match environment variables

### API rate limit errors
- Set appropriate rate limit override environment variables
- Check current limits: `docker-compose exec data-ingestion env | grep RATE_LIMIT`