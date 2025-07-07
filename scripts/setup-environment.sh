#!/bin/bash
# Setup environment configuration for Neural Trader

echo "🔧 Neural Trader Environment Setup"
echo "================================="

# Function to generate secure password
generate_password() {
    openssl rand -base64 32 | tr -d "=+/" | cut -c1-32
}

# Function to generate secure key
generate_key() {
    openssl rand -hex 32
}

# Create .env from .env.example
if [ ! -f .env ]; then
    echo "📋 Creating .env from .env.example..."
    cp .env.example .env
    
    # Replace password placeholders with environment variables
    sed -i "s/CHANGE_THIS_PASSWORD/\${POSTGRES_PASSWORD}/g" .env
    sed -i "s/CHANGE_THIS_REDIS_PASSWORD/\${REDIS_PASSWORD}/g" .env
    sed -i "s/CHANGE_THIS_GRAFANA_PASSWORD/\${GRAFANA_ADMIN_PASSWORD}/g" .env
    
    # Replace other secret placeholders
    sed -i "s/your_jwt_secret_key_minimum_32_characters_long_and_random/\${JWT_SECRET}/g" .env
    sed -i "s/your_encryption_key_32_characters_long_and_random/\${ENCRYPTION_KEY}/g" .env
    sed -i "s/your_session_secret_key_minimum_32_characters_long/\${SESSION_SECRET}/g" .env
    sed -i "s/your_backup_encryption_key_32_characters_long/\${BACKUP_ENCRYPTION_KEY}/g" .env
    
    # Replace API key placeholders
    sed -i "s/your_alpha_vantage_api_key_here/\${ALPHA_VANTAGE_API_KEY}/g" .env
    sed -i "s/your_finnhub_api_key_here/\${FINNHUB_API_KEY}/g" .env
    sed -i "s/your_polygon_api_key_here/\${POLYGON_API_KEY}/g" .env
    sed -i "s/your_iex_cloud_api_key_here/\${IEX_CLOUD_API_KEY}/g" .env
    
    # Update DATABASE_URL to use environment variable
    sed -i "s|postgresql://neural_trader:.*@|postgresql://neural_trader:\${POSTGRES_PASSWORD}@|g" .env
    
    echo "✅ .env file created with environment variable references"
else
    echo "⚠️  .env file already exists, skipping creation"
fi

# Generate secrets for local development (if not in Codespaces)
if [ -z "$CODESPACES" ]; then
    echo ""
    echo "🔐 Generating secrets for local development..."
    echo "============================================"
    
    cat > .env.secrets << EOF
# Generated secrets for local development - $(date)
# ⚠️  DO NOT COMMIT THIS FILE!

# Passwords
export POSTGRES_PASSWORD=$(generate_password)
export REDIS_PASSWORD=$(generate_password)
export PGADMIN_DEFAULT_PASSWORD=$(generate_password)
export GRAFANA_ADMIN_PASSWORD=$(generate_password)

# Security Keys
export JWT_SECRET=$(generate_key)
export ENCRYPTION_KEY=$(generate_key)
export SESSION_SECRET=$(generate_key)
export BACKUP_ENCRYPTION_KEY=$(generate_key)

# SMTP (if needed)
export SMTP_PASSWORD="your_smtp_password_here"

# AWS (if needed)
export AWS_SECRET_ACCESS_KEY="your_aws_secret_here"

# API Keys (replace with actual keys)
export ALPHA_VANTAGE_API_KEY="demo"
export FINNHUB_API_KEY="your_key_here"
export POLYGON_API_KEY="your_key_here"
export IEX_CLOUD_API_KEY="your_key_here"
EOF

    echo "✅ Local secrets generated in .env.secrets"
    echo "   Run: source .env.secrets"
else
    echo ""
    echo "🌐 Running in GitHub Codespaces"
    echo "   Secrets should be configured in GitHub Settings"
fi

# Create a secrets checklist
echo ""
echo "📋 Required Codespaces Secrets Checklist:"
echo "========================================="
cat > SECRETS_CHECKLIST.md << 'EOF'
# GitHub Codespaces Secrets Setup

## Required Secrets (Add to https://github.com/settings/codespaces)

### 🔴 Critical (Passwords & Keys)
- [ ] `POSTGRES_PASSWORD` - PostgreSQL password
- [ ] `REDIS_PASSWORD` - Redis password
- [ ] `JWT_SECRET` - JWT signing key (32+ chars)
- [ ] `ENCRYPTION_KEY` - Data encryption key (32+ chars)
- [ ] `SESSION_SECRET` - Session key (32+ chars)
- [ ] `BACKUP_ENCRYPTION_KEY` - Backup encryption key

### 🟡 Admin Interfaces
- [ ] `PGADMIN_DEFAULT_PASSWORD` - PgAdmin password
- [ ] `GRAFANA_ADMIN_PASSWORD` - Grafana password

### 🟢 API Keys (Get free keys from providers)
- [ ] `ALPHA_VANTAGE_API_KEY` - https://www.alphavantage.co/support/#api-key
- [ ] `FINNHUB_API_KEY` - https://finnhub.io/register
- [ ] `POLYGON_API_KEY` - https://polygon.io/dashboard/api-keys
- [ ] `IEX_CLOUD_API_KEY` - https://iexcloud.io/console/tokens

### 🔵 Optional Services
- [ ] `FRED_API_KEY` - Federal Reserve data
- [ ] `NEWSAPI_KEY` - News sentiment
- [ ] `REDDIT_CLIENT_ID` - Reddit API
- [ ] `REDDIT_CLIENT_SECRET` - Reddit API
- [ ] `SMTP_PASSWORD` - Email notifications
- [ ] `AWS_SECRET_ACCESS_KEY` - S3 backups
- [ ] `SLACK_WEBHOOK_URL` - Slack alerts

## Quick Test

After setting secrets, restart Codespace and run:
```bash
./scripts/test-secrets.sh
```
EOF

echo "✅ Created SECRETS_CHECKLIST.md"
echo ""
echo "🚀 Next Steps:"
echo "1. Add secrets to GitHub Codespaces Settings"
echo "2. Restart your Codespace"
echo "3. Run: ./scripts/test-secrets.sh"
echo "4. Start services: docker-compose up -d"