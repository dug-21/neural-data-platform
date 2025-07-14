# Required Environment Variables for Neural Trader

## 🔐 Security Notice
These environment variables MUST be set in your shell/system environment before running docker-compose.
DO NOT store actual passwords in .env files or any files on disk.

## 📋 Required Variables

### Core Database Configuration
```bash
export POSTGRES_USER=neural_trader
export POSTGRES_PASSWORD=<your-secure-password>
export POSTGRES_DB=neural_trader_db
```

### Application Configuration
```bash
export LOG_LEVEL=INFO
export GRAFANA_PASSWORD=<your-secure-grafana-password>
```

### Trading Configuration
```bash
export TRADING_SYMBOLS_PRIMARY=AAPL,MSFT,GOOGL,AMZN,NVDA,DDOG
export UPDATE_INTERVAL=60
export PRIMARY_PROVIDER=alpaca
export USE_SIMPLE_MODE=true
```

### API Keys (Required for data providers)
```bash
export ALPACA_API_KEY=<your-alpaca-key>
export ALPACA_API_SECRET=<your-alpaca-secret>
export FINNHUB_API_KEY=<your-finnhub-key>
export ALPHA_VANTAGE_API_KEY=<your-alpha-vantage-key>
export ALPHA_ADVANTAGE_API_KEY=<your-alpha-advantage-key>
export IEX_CLOUD_API_KEY=<your-iex-cloud-key>
export POLYGON_API_KEY=<your-polygon-key>
export QUANDL_API_KEY=<your-quandl-key>
export FRED_API_KEY=<your-fred-key>
export NASDAQ_API_KEY=<your-nasdaq-key>
export NEWSAPI_KEY=<your-newsapi-key>
export REDDIT_CLIENT_ID=<your-reddit-client-id>
export REDDIT_CLIENT_SECRET=<your-reddit-client-secret>
```

## 🚀 Quick Start

1. Set all required environment variables in your shell:
   ```bash
   export POSTGRES_USER=neural_trader
   export POSTGRES_PASSWORD=your_secure_password_here
   export POSTGRES_DB=neural_trader_db
   export LOG_LEVEL=INFO
   export GRAFANA_PASSWORD=your_grafana_password_here
   # ... set all other required variables
   ```

2. Verify variables are set:
   ```bash
   env | grep -E "POSTGRES_|GRAFANA_|LOG_LEVEL"
   ```

3. Validate configuration (no fallbacks will be used):
   ```bash
   docker-compose -f docker-compose.prod.yml config | grep -E "CHANGE_THIS|REQUIRED_SET" && echo "ERROR: Placeholder values detected" && exit 1
   ```

4. Start the system:
   ```bash
   docker-compose -f docker-compose.prod.yml up -d
   ```

## 🔒 Security Best Practices

1. **Never commit real passwords** to version control
2. **Use a password manager** or secure vault for credentials
3. **Rotate passwords regularly**
4. **Use strong, unique passwords** for each service
5. **Set variables in CI/CD** environment for automated deployments

## ⚠️ Important Notes

- All variables listed above are REQUIRED - the system will fail to start if any are missing
- There are NO fallback defaults - this is by design for security
- The .env file should only contain placeholder values for documentation
- Real credentials should only exist in environment variables