#!/bin/bash
# Test that Codespaces secrets are properly configured

echo "🔍 Testing Codespaces Secrets Configuration"
echo "=========================================="
echo ""

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to check if a secret is set
check_secret() {
    local secret_name=$1
    local required=$2
    local description=$3
    
    if [ -z "${!secret_name}" ]; then
        if [ "$required" = "required" ]; then
            echo -e "${RED}❌ $secret_name${NC} - NOT SET (Required: $description)"
            return 1
        else
            echo -e "${YELLOW}⚠️  $secret_name${NC} - NOT SET (Optional: $description)"
            return 0
        fi
    else
        # Show length but not the actual value for security
        local length=${#!secret_name}
        echo -e "${GREEN}✅ $secret_name${NC} - SET (length: $length chars)"
        return 0
    fi
}

# Track if all required secrets are set
all_required_set=true

echo "🔐 Database Secrets:"
check_secret "POSTGRES_PASSWORD" "required" "PostgreSQL password" || all_required_set=false
check_secret "POSTGRES_USER" "optional" "PostgreSQL user (default: neural_trader)"
check_secret "POSTGRES_DB" "optional" "PostgreSQL database (default: neural_trader_db)"

echo ""
echo "💾 Cache Secrets:"
check_secret "REDIS_PASSWORD" "required" "Redis password" || all_required_set=false

echo ""
echo "📊 API Keys (Optional):"
check_secret "ALPHA_VANTAGE_API_KEY" "optional" "Alpha Vantage market data"
check_secret "POLYGON_API_KEY" "optional" "Polygon.io market data"
check_secret "FINNHUB_API_KEY" "optional" "Finnhub financial data"
check_secret "FRED_API_KEY" "optional" "Federal Reserve economic data"
check_secret "NEWSAPI_KEY" "optional" "News API for sentiment"
check_secret "REDDIT_CLIENT_ID" "optional" "Reddit API client ID"
check_secret "REDDIT_CLIENT_SECRET" "optional" "Reddit API client secret"

echo ""
echo "🎛️ Admin Interfaces (Optional):"
check_secret "PGADMIN_DEFAULT_PASSWORD" "optional" "PgAdmin web interface"
check_secret "GRAFANA_ADMIN_PASSWORD" "optional" "Grafana monitoring"

echo ""
echo "=========================================="

if [ "$all_required_set" = true ]; then
    echo -e "${GREEN}✅ All required secrets are configured!${NC}"
    echo ""
    echo "📝 Next steps:"
    echo "1. Start services: sudo docker-compose up -d"
    echo "2. Run tests: cargo test"
    echo "3. Start MCP server: cargo run --bin mcp_server"
else
    echo -e "${RED}❌ Some required secrets are missing!${NC}"
    echo ""
    echo "📝 To fix:"
    echo "1. Run: ./scripts/generate-passwords.sh"
    echo "2. Go to: https://github.com/settings/codespaces"
    echo "3. Add the missing secrets"
    echo "4. Restart your Codespace"
fi

echo ""
echo "💡 Tip: You can also create a local .env file for testing:"
echo "   cp .env.generated .env"
echo "   source .env"