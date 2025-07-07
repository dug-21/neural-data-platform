#!/bin/bash
# Create Podman secrets for Neural Trader
# This script creates all necessary secrets from environment variables

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Creating Podman secrets...${NC}"

# Function to create or update a secret
create_secret() {
    local secret_name=$1
    local secret_value=$2
    
    if [[ -z "${secret_value}" ]]; then
        echo -e "${YELLOW}Warning: Empty value for secret ${secret_name}${NC}"
        return 1
    fi
    
    # Remove existing secret if it exists
    if podman secret exists "${secret_name}" 2>/dev/null; then
        echo -e "${YELLOW}Updating existing secret: ${secret_name}${NC}"
        podman secret rm "${secret_name}" >/dev/null
    fi
    
    # Create new secret
    echo -n "${secret_value}" | podman secret create "${secret_name}" -
    echo -e "${GREEN}Created secret: ${secret_name}${NC}"
}

# Function to generate a random password
generate_password() {
    openssl rand -base64 32 | tr -d "=+/" | cut -c1-25
}

# Load environment variables if .env file exists
if [[ -f "${PROJECT_ROOT}/.env" ]]; then
    echo -e "${BLUE}Loading environment variables from .env file...${NC}"
    set -a
    source "${PROJECT_ROOT}/.env"
    set +a
fi

# Create main secrets
echo -e "${BLUE}Creating database and service secrets...${NC}"

# PostgreSQL credentials
POSTGRES_USER="${POSTGRES_USER:-neural_trader}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-$(generate_password)}"

# Redis password
REDIS_PASSWORD="${REDIS_PASSWORD:-$(generate_password)}"

# Admin passwords
GRAFANA_ADMIN_PASSWORD="${GRAFANA_ADMIN_PASSWORD:-$(generate_password)}"
PGADMIN_DEFAULT_PASSWORD="${PGADMIN_DEFAULT_PASSWORD:-$(generate_password)}"

# Create the main secrets bundle
cat > /tmp/neural-trader-secrets.yml <<EOF
postgres-user: ${POSTGRES_USER}
postgres-password: ${POSTGRES_PASSWORD}
redis-password: ${REDIS_PASSWORD}
grafana-password: ${GRAFANA_ADMIN_PASSWORD}
pgadmin-password: ${PGADMIN_DEFAULT_PASSWORD}
EOF

podman secret exists neural-trader-secrets 2>/dev/null && \
    podman secret rm neural-trader-secrets >/dev/null
podman secret create neural-trader-secrets /tmp/neural-trader-secrets.yml
rm -f /tmp/neural-trader-secrets.yml

echo -e "${GREEN}Created main secrets bundle${NC}"

# Create API keys secret bundle (if any keys are set)
echo -e "${BLUE}Creating API keys secret bundle...${NC}"

API_KEYS_FILE="/tmp/neural-trader-api-keys.yml"
> "${API_KEYS_FILE}"

# Add API keys if they exist
[[ -n "${IEX_CLOUD_API_KEY:-}" ]] && echo "iex-cloud-api-key: ${IEX_CLOUD_API_KEY}" >> "${API_KEYS_FILE}"
[[ -n "${ALPHA_VANTAGE_API_KEY:-}" ]] && echo "alpha-vantage-api-key: ${ALPHA_VANTAGE_API_KEY}" >> "${API_KEYS_FILE}"
[[ -n "${POLYGON_API_KEY:-}" ]] && echo "polygon-api-key: ${POLYGON_API_KEY}" >> "${API_KEYS_FILE}"
[[ -n "${FINNHUB_API_KEY:-}" ]] && echo "finnhub-api-key: ${FINNHUB_API_KEY}" >> "${API_KEYS_FILE}"
[[ -n "${FRED_API_KEY:-}" ]] && echo "fred-api-key: ${FRED_API_KEY}" >> "${API_KEYS_FILE}"
[[ -n "${REDDIT_CLIENT_ID:-}" ]] && echo "reddit-client-id: ${REDDIT_CLIENT_ID}" >> "${API_KEYS_FILE}"
[[ -n "${REDDIT_CLIENT_SECRET:-}" ]] && echo "reddit-client-secret: ${REDDIT_CLIENT_SECRET}" >> "${API_KEYS_FILE}"
[[ -n "${QUANDL_API_KEY:-}" ]] && echo "quandl-api-key: ${QUANDL_API_KEY}" >> "${API_KEYS_FILE}"
[[ -n "${NEWSAPI_KEY:-}" ]] && echo "newsapi-key: ${NEWSAPI_KEY}" >> "${API_KEYS_FILE}"
[[ -n "${YAHOO_API_KEY:-}" ]] && echo "yahoo-api-key: ${YAHOO_API_KEY}" >> "${API_KEYS_FILE}"
[[ -n "${NASDAQ_API_KEY:-}" ]] && echo "nasdaq-api-key: ${NASDAQ_API_KEY}" >> "${API_KEYS_FILE}"

# Only create the secret if we have at least one API key
if [[ -s "${API_KEYS_FILE}" ]]; then
    podman secret exists neural-trader-api-keys 2>/dev/null && \
        podman secret rm neural-trader-api-keys >/dev/null
    podman secret create neural-trader-api-keys "${API_KEYS_FILE}"
    echo -e "${GREEN}Created API keys secret bundle${NC}"
else
    echo -e "${YELLOW}No API keys found, skipping API keys secret${NC}"
fi

rm -f "${API_KEYS_FILE}"

# Save credentials to a secure file for reference (only if they were generated)
CREDS_FILE="${PROJECT_ROOT}/.podman-credentials"
if [[ "${POSTGRES_PASSWORD}" != "${POSTGRES_PASSWORD:-}" ]] || \
   [[ "${REDIS_PASSWORD}" != "${REDIS_PASSWORD:-}" ]] || \
   [[ "${GRAFANA_ADMIN_PASSWORD}" != "${GRAFANA_ADMIN_PASSWORD:-}" ]] || \
   [[ "${PGADMIN_DEFAULT_PASSWORD}" != "${PGADMIN_DEFAULT_PASSWORD:-}" ]]; then
    
    echo -e "${BLUE}Saving generated credentials to ${CREDS_FILE}${NC}"
    cat > "${CREDS_FILE}" <<EOF
# Generated Podman Credentials for Neural Trader
# Created: $(date)
# KEEP THIS FILE SECURE!

# Database
POSTGRES_USER=${POSTGRES_USER}
POSTGRES_PASSWORD=${POSTGRES_PASSWORD}

# Redis
REDIS_PASSWORD=${REDIS_PASSWORD}

# Admin UIs
GRAFANA_ADMIN_USER=admin
GRAFANA_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD}
PGADMIN_DEFAULT_EMAIL=admin@neuraltrader.local
PGADMIN_DEFAULT_PASSWORD=${PGADMIN_DEFAULT_PASSWORD}

# Connection Examples:
# PostgreSQL: psql -h localhost -U ${POSTGRES_USER} -d neural_trader_db
# Redis: redis-cli -h localhost -a ${REDIS_PASSWORD}
# Grafana: http://localhost:3000 (admin/${GRAFANA_ADMIN_PASSWORD})
# pgAdmin: http://localhost:8082 (admin@neuraltrader.local/${PGADMIN_DEFAULT_PASSWORD})
EOF
    
    chmod 600 "${CREDS_FILE}"
    echo -e "${GREEN}Credentials saved to ${CREDS_FILE}${NC}"
    echo -e "${YELLOW}Make sure to keep this file secure!${NC}"
fi

# List created secrets
echo -e "${BLUE}Created secrets:${NC}"
podman secret ls --format "table {{.Name}}\t{{.CreatedAt}}" | grep neural-trader || true

echo -e "${GREEN}All secrets created successfully!${NC}"