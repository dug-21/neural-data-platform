#!/bin/bash
# Check if all required environment variables are set for Neural Trader

echo "Neural Trader Environment Check"
echo "==============================="
echo

# Define required variables
REQUIRED_SECRETS=(
    "POSTGRES_PASSWORD"
    "REDIS_PASSWORD"
    "GRAFANA_ADMIN_PASSWORD"
)

REQUIRED_API_KEYS=(
    "IEX_CLOUD_API_KEY"
    "ALPHA_VANTAGE_API_KEY"
    "POLYGON_API_KEY"
    "FINNHUB_API_KEY"
    "FRED_API_KEY"
    "REDDIT_CLIENT_ID"
    "REDDIT_CLIENT_SECRET"
    "QUANDL_API_KEY"
    "NEWSAPI_KEY"
    "YAHOO_API_KEY"
    "NASDAQ_API_KEY"
)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Check function
check_var() {
    local var_name=$1
    local var_value="${!var_name}"
    
    if [ -z "$var_value" ]; then
        echo -e "${RED}✗ $var_name${NC} - NOT SET"
        return 1
    else
        # Show partial value for verification (first 3 chars only)
        local preview="${var_value:0:3}..."
        echo -e "${GREEN}✓ $var_name${NC} - SET (${preview})"
        return 0
    fi
}

# Track failures
MISSING_COUNT=0

echo "Checking Required Passwords:"
echo "----------------------------"
for var in "${REQUIRED_SECRETS[@]}"; do
    check_var "$var" || ((MISSING_COUNT++))
done

echo
echo "Checking API Keys:"
echo "------------------"
for var in "${REQUIRED_API_KEYS[@]}"; do
    check_var "$var" || ((MISSING_COUNT++))
done

echo
echo "Checking Optional Configuration:"
echo "--------------------------------"
# Optional vars
OPTIONAL_VARS=(
    "LOG_LEVEL"
    "POSTGRES_USER"
    "POSTGRES_DB"
    "RUST_LOG"
    "RUST_BACKTRACE"
)

for var in "${OPTIONAL_VARS[@]}"; do
    if [ -z "${!var}" ]; then
        echo -e "${YELLOW}○ $var${NC} - Using default"
    else
        echo -e "${GREEN}✓ $var${NC} - SET (${!var})"
    fi
done

echo
echo "Summary:"
echo "--------"
if [ $MISSING_COUNT -eq 0 ]; then
    echo -e "${GREEN}✓ All required environment variables are set!${NC}"
    echo
    echo "You can now run: docker-compose up -d"
else
    echo -e "${RED}✗ Missing $MISSING_COUNT required environment variables${NC}"
    echo
    echo "To generate passwords, run:"
    echo "  source ./scripts/setup-docker-env.sh"
    echo
    echo "Then set your API keys as shown above."
    exit 1
fi

# Check for .env files that might contain secrets
echo
echo "Security Check:"
echo "--------------"
if [ -f .env ]; then
    # Check if .env contains any secret patterns
    if grep -qE "(PASSWORD|SECRET|API_KEY)" .env 2>/dev/null; then
        echo -e "${RED}⚠ WARNING: .env file may contain secrets!${NC}"
        echo "Secrets should only be set as environment variables"
    else
        echo -e "${GREEN}✓ .env file appears safe (no secrets detected)${NC}"
    fi
else
    echo "○ No .env file found"
fi

# Show docker-compose command with current environment
echo
echo "To run with current environment:"
echo "--------------------------------"
echo "docker-compose up -d"
echo
echo "To run with specific compose file:"
echo "docker-compose -f docker-compose.yml up -d"