#!/bin/bash

# Generate development secrets for Neural Trader
# This is for DEVELOPMENT ONLY - do not use in production!

echo "🔐 Generating development secrets..."
echo ""
echo "⚠️  WARNING: These are for development only!"
echo "⚠️  Use proper secrets management in production!"
echo ""

# Generate random passwords
generate_password() {
    openssl rand -base64 32 | tr -d "=+/" | cut -c1-25
}

# Generate secrets
POSTGRES_PASSWORD=$(generate_password)
REDIS_PASSWORD=$(generate_password)
JWT_SECRET=$(generate_password)
GRAFANA_ADMIN_PASSWORD=$(generate_password)
ENCRYPTION_KEY=$(generate_password)
SESSION_SECRET=$(generate_password)

# Display the secrets
echo "Add these to your shell environment:"
echo ""
echo "# Database passwords"
echo "export POSTGRES_PASSWORD='$POSTGRES_PASSWORD'"
echo "export REDIS_PASSWORD='$REDIS_PASSWORD'"
echo ""
echo "# Security secrets"
echo "export JWT_SECRET='$JWT_SECRET'"
echo "export ENCRYPTION_KEY='$ENCRYPTION_KEY'"
echo "export SESSION_SECRET='$SESSION_SECRET'"
echo ""
echo "# Grafana admin"
echo "export GRAFANA_ADMIN_PASSWORD='$GRAFANA_ADMIN_PASSWORD'"
echo ""
echo "# Optional: Email configuration (if using notifications)"
echo "# export SMTP_HOST='smtp.gmail.com'"
echo "# export SMTP_PORT='587'"
echo "# export SMTP_USER='your-email@gmail.com'"
echo "# export SMTP_PASSWORD='your-app-password'"
echo "# export ALERT_EMAIL='alerts@example.com'"
echo ""

# Optionally save to a file
read -p "Save to ~/.neural_trader_dev_secrets? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    cat > ~/.neural_trader_dev_secrets << EOF
# Neural Trader Development Secrets
# Generated on $(date)
# Source this file: source ~/.neural_trader_dev_secrets

# Database passwords
export POSTGRES_PASSWORD='$POSTGRES_PASSWORD'
export REDIS_PASSWORD='$REDIS_PASSWORD'

# Security secrets
export JWT_SECRET='$JWT_SECRET'
export ENCRYPTION_KEY='$ENCRYPTION_KEY'
export SESSION_SECRET='$SESSION_SECRET'

# Grafana admin
export GRAFANA_ADMIN_PASSWORD='$GRAFANA_ADMIN_PASSWORD'
EOF
    
    chmod 600 ~/.neural_trader_dev_secrets
    echo "✅ Saved to ~/.neural_trader_dev_secrets"
    echo "   Run: source ~/.neural_trader_dev_secrets"
fi

echo ""
echo "✅ Development secrets generated!"
echo ""
echo "Next steps:"
echo "1. Copy and paste the export commands above into your shell"
echo "2. Or source the saved file: source ~/.neural_trader_dev_secrets"
echo "3. Add your API keys (FINNHUB_API_KEY, etc.)"
echo "4. Run: ./scripts/start_full_stock_simulation_optimized.sh"