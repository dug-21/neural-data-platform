#!/bin/bash

# Setup Podman secrets for Neural Trader API keys
# This keeps your API keys secure and off disk

set -e

echo "🔐 Neural Trader Podman Secrets Setup"
echo "====================================="
echo ""
echo "This script will help you securely store API keys as Podman secrets."
echo "The keys will be available to containers but not stored on disk."
echo ""

# Function to create or update a secret
create_secret() {
    local secret_name=$1
    local prompt_text=$2
    
    # Check if secret already exists
    if podman secret ls | grep -q "^${secret_name}"; then
        echo "⚠️  Secret '${secret_name}' already exists."
        read -p "Do you want to update it? (y/n) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo "Skipping ${secret_name}..."
            return
        fi
        # Remove existing secret
        podman secret rm ${secret_name} >/dev/null 2>&1
    fi
    
    # Prompt for the secret value
    echo ""
    echo "${prompt_text}"
    read -s -p "Enter value (hidden): " secret_value
    echo
    
    if [ -z "$secret_value" ]; then
        echo "⚠️  No value entered, skipping ${secret_name}"
        return
    fi
    
    # Create the secret
    echo -n "$secret_value" | podman secret create ${secret_name} -
    echo "✅ Created secret: ${secret_name}"
}

# Create secrets for each API key
echo "📊 Setting up API key secrets..."
echo "You can set up one or more providers. Press Enter to skip any you don't have."

create_secret "finnhub_api_key" "Finnhub API Key (Best - 60 calls/min free):"
create_secret "alpha_vantage_api_key" "Alpha Vantage API Key:"
create_secret "iex_cloud_api_key" "IEX Cloud API Key:"
create_secret "polygon_api_key" "Polygon.io API Key:"

# Create other important secrets
echo ""
echo "🔐 Setting up other secrets..."
create_secret "grafana_admin_password" "Grafana Admin Password (default: admin):"

# List all secrets
echo ""
echo "📋 Current Podman secrets:"
podman secret ls

echo ""
echo "✅ Secrets setup complete!"
echo ""
echo "These secrets will be automatically available to your containers"
echo "as environment variables when using the podman-compose configuration."
echo ""
echo "To use these secrets in docker-compose.yml, add under each service:"
echo ""
echo "  secrets:"
echo "    - finnhub_api_key"
echo "    - alpha_vantage_api_key"
echo "    # etc..."
echo ""
echo "And at the top level of docker-compose.yml:"
echo ""
echo "secrets:"
echo "  finnhub_api_key:"
echo "    external: true"
echo "  alpha_vantage_api_key:"
echo "    external: true"
echo "  # etc..."