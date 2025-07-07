#!/bin/bash
# Generate secure passwords for PostgreSQL and Redis

echo "🔐 Generating secure passwords for Neural Trader"
echo "=============================================="

# Function to generate secure password
generate_password() {
    # Use /dev/urandom for cryptographically secure randomness
    # - 32 characters long
    # - Mix of letters, numbers, and safe special characters
    # - Avoids problematic characters like quotes, backslashes
    openssl rand -base64 32 | tr -d "=+/" | cut -c1-32
}

# Generate passwords
POSTGRES_PASSWORD=$(generate_password)
REDIS_PASSWORD=$(generate_password)
PGADMIN_PASSWORD=$(generate_password)
GRAFANA_PASSWORD=$(generate_password)

echo ""
echo "📋 Generated Passwords (save these securely!):"
echo "=============================================="
echo ""
echo "# PostgreSQL"
echo "POSTGRES_PASSWORD=$POSTGRES_PASSWORD"
echo ""
echo "# Redis"
echo "REDIS_PASSWORD=$REDIS_PASSWORD"
echo ""
echo "# PgAdmin (optional)"
echo "PGADMIN_DEFAULT_PASSWORD=$PGADMIN_PASSWORD"
echo ""
echo "# Grafana (optional)"
echo "GRAFANA_ADMIN_PASSWORD=$GRAFANA_PASSWORD"
echo ""
echo "=============================================="
echo ""
echo "🔧 To add these to GitHub Codespaces Secrets:"
echo ""
echo "1. Go to: https://github.com/settings/codespaces"
echo "2. Click 'New secret' for each password"
echo "3. Use the exact variable names above"
echo "4. Paste the generated password values"
echo ""
echo "📝 For local .env file (development only):"
echo ""
cat << EOF > .env.generated
# Generated passwords - $(date)
# ⚠️  DO NOT COMMIT THIS FILE TO GIT!

# Database
POSTGRES_PASSWORD=$POSTGRES_PASSWORD
POSTGRES_USER=neural_trader
POSTGRES_DB=neural_trader_db
DATABASE_URL=postgresql://neural_trader:$POSTGRES_PASSWORD@localhost:5432/neural_trader_db

# Redis
REDIS_PASSWORD=$REDIS_PASSWORD
REDIS_URL=redis://:$REDIS_PASSWORD@localhost:6379

# Optional services
PGADMIN_DEFAULT_PASSWORD=$PGADMIN_PASSWORD
GRAFANA_ADMIN_PASSWORD=$GRAFANA_PASSWORD
EOF

echo "✅ Passwords saved to: .env.generated"
echo ""
echo "🚨 Security Best Practices:"
echo "   - NEVER commit passwords to Git"
echo "   - Use Codespaces Secrets for cloud development"
echo "   - Use environment variables in production"
echo "   - Rotate passwords regularly"
echo "   - Use different passwords for each environment"