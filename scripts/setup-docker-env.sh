#!/bin/bash
# Setup script for Neural Trader Docker environment variables
# This script helps generate secure passwords and exports them to the current shell
# NO SECRETS ARE WRITTEN TO DISK

set -e

echo "Neural Trader Docker Environment Setup"
echo "====================================="
echo

# Function to generate secure password
generate_password() {
    openssl rand -base64 32 | tr -d "=+/" | cut -c1-25
}

# Check if we can export to parent shell
if [ "${BASH_SOURCE[0]}" == "${0}" ]; then
    echo "ERROR: This script must be sourced, not executed directly"
    echo "Usage: source $0"
    exit 1
fi

# Generate database passwords
echo "Generating secure passwords..."
export POSTGRES_PASSWORD=$(generate_password)
export REDIS_PASSWORD=$(generate_password)
export GRAFANA_ADMIN_PASSWORD=$(generate_password)

# Optional development passwords
if [ "$1" == "--dev" ]; then
    export PGADMIN_DEFAULT_PASSWORD=$GRAFANA_ADMIN_PASSWORD
fi

# Show what needs to be set
echo ""
echo "✓ Generated and exported secure passwords to environment"
echo ""
echo "Required API Keys - You must set these yourself:"
echo "  export IEX_CLOUD_API_KEY='your-key'"
echo "  export ALPHA_VANTAGE_API_KEY='your-key'"
echo "  export POLYGON_API_KEY='your-key'"
echo "  export FINNHUB_API_KEY='your-key'"
echo "  export FRED_API_KEY='your-key'"
echo "  export REDDIT_CLIENT_ID='your-client-id'"
echo "  export REDDIT_CLIENT_SECRET='your-secret'"
echo "  export QUANDL_API_KEY='your-key'"
echo "  export NEWSAPI_KEY='your-key'"
echo "  export YAHOO_API_KEY='your-key'"
echo "  export NASDAQ_API_KEY='your-key'"
echo ""
echo "To verify all required variables are set:"
echo "  ./scripts/check-env.sh"
echo ""
echo "To start services:"
echo "  docker-compose up -d"
echo ""
echo "SECURITY NOTE: These passwords exist only in memory"
echo "They will be lost when you close this shell session"