#!/bin/bash

# Script to help users set up environment variables for stock trading
# This keeps sensitive data out of files

echo "📈 Neural Trader Stock Trading Environment Setup"
echo "=============================================="
echo ""
echo "This script will help you set up environment variables"
echo "for stock trading simulation. Your API keys will be"
echo "stored in your shell environment, not in any files."
echo ""

# Function to prompt for a value with a default
prompt_with_default() {
    local prompt="$1"
    local default="$2"
    local var_name="$3"
    
    if [ -n "$default" ]; then
        read -p "$prompt [$default]: " value
        value="${value:-$default}"
    else
        read -p "$prompt: " value
    fi
    
    if [ -n "$value" ]; then
        echo "export $var_name=\"$value\""
    fi
}

# Check if .env.generated exists for passwords
if [ -f .env.generated ]; then
    echo "✅ Found .env.generated with secure passwords"
    echo ""
    
    # Extract passwords
    POSTGRES_PASS=$(grep "^POSTGRES_PASSWORD=" .env.generated | cut -d'=' -f2)
    REDIS_PASS=$(grep "^REDIS_PASSWORD=" .env.generated | cut -d'=' -f2)
    GRAFANA_PASS=$(grep "^GRAFANA_ADMIN_PASSWORD=" .env.generated | cut -d'=' -f2)
    
    echo "The following secure passwords will be used:"
    echo "  - PostgreSQL: ${POSTGRES_PASS:0:8}..."
    echo "  - Redis: ${REDIS_PASS:0:8}..."
    echo "  - Grafana: ${GRAFANA_PASS:0:8}..."
    echo ""
else
    echo "⚠️  Warning: .env.generated not found"
    echo "Run ./scripts/generate-passwords.sh first to create secure passwords"
    exit 1
fi

echo "Now let's set up your API keys..."
echo ""
echo "Choose your primary data provider:"
echo "1) Finnhub (Recommended - 60 calls/min)"
echo "2) Alpha Vantage (500 calls/day)"
echo "3) IEX Cloud (50k messages/month)"
echo "4) Polygon (5 calls/min free)"
echo ""
read -p "Enter choice [1-4]: " provider_choice

case $provider_choice in
    1)
        PRIMARY_PROVIDER="finnhub"
        echo ""
        echo "Get your FREE Finnhub API key from: https://finnhub.io/register"
        prompt_with_default "Enter your Finnhub API key" "" "FINNHUB_API_KEY"
        ;;
    2)
        PRIMARY_PROVIDER="alpha_vantage"
        echo ""
        echo "Get your FREE Alpha Vantage key from: https://www.alphavantage.co/support/#api-key"
        prompt_with_default "Enter your Alpha Vantage API key" "" "ALPHA_VANTAGE_API_KEY"
        ;;
    3)
        PRIMARY_PROVIDER="iex_cloud"
        echo ""
        echo "Get your FREE IEX Cloud key from: https://iexcloud.io/console/tokens"
        prompt_with_default "Enter your IEX Cloud API key" "" "IEX_CLOUD_API_KEY"
        ;;
    4)
        PRIMARY_PROVIDER="polygon"
        echo ""
        echo "Get your FREE Polygon key from: https://polygon.io/dashboard/api-keys"
        prompt_with_default "Enter your Polygon API key" "" "POLYGON_API_KEY"
        ;;
    *)
        echo "Invalid choice. Exiting."
        exit 1
        ;;
esac

echo ""
echo "Would you like to add additional data providers? (y/n)"
read -p "> " add_more

if [[ $add_more =~ ^[Yy]$ ]]; then
    echo ""
    echo "Optional providers (press Enter to skip):"
    prompt_with_default "FRED API key (economic data)" "" "FRED_API_KEY"
    prompt_with_default "NewsAPI key (news sentiment)" "" "NEWSAPI_KEY"
    prompt_with_default "Reddit Client ID" "" "REDDIT_CLIENT_ID"
    prompt_with_default "Reddit Client Secret" "" "REDDIT_CLIENT_SECRET"
fi

# Generate the export commands
echo ""
echo "========================================"
echo "Add these to your shell profile (~/.bashrc or ~/.zshrc):"
echo ""
echo "# Neural Trader Stock Trading Environment"
echo "export PRIMARY_PROVIDER=\"$PRIMARY_PROVIDER\""
echo "export POSTGRES_PASSWORD=\"$POSTGRES_PASS\""
echo "export REDIS_PASSWORD=\"$REDIS_PASS\""
echo "export GRAFANA_ADMIN_PASSWORD=\"$GRAFANA_PASS\""

# Security tokens (generate if not exist)
JWT_SECRET=$(openssl rand -base64 32 | tr -d "=+/" | cut -c1-32)
ENCRYPTION_KEY=$(openssl rand -hex 32 | cut -c1-32)
SESSION_SECRET=$(openssl rand -base64 32 | tr -d "=+/" | cut -c1-32)

echo "export JWT_SECRET=\"$JWT_SECRET\""
echo "export ENCRYPTION_KEY=\"$ENCRYPTION_KEY\""
echo "export SESSION_SECRET=\"$SESSION_SECRET\""

# Show the command outputs from earlier
prompt_with_default | while read line; do
    [ -n "$line" ] && echo "$line"
done

echo ""
echo "========================================"
echo ""
echo "Or run this command to set them for the current session:"
echo ""
echo "source <(./scripts/setup_stock_env.sh)"
echo ""
echo "Then run: ./scripts/start_full_stock_simulation.sh"