# Setting Up Secrets in GitHub Codespaces

## 🔐 Password Generation Best Practices

### For Development/Testing:
- Use the `generate-passwords.sh` script to create secure passwords
- 32 characters, alphanumeric, cryptographically random
- Different passwords for each service

### For Production:
- Use a proper secrets management service (HashiCorp Vault, AWS Secrets Manager, etc.)
- Enable password rotation
- Use even longer passwords (64+ characters)
- Consider using certificates for database authentication

## 📋 Step-by-Step Codespaces Setup

### 1. Generate Passwords
```bash
./scripts/generate-passwords.sh
```

### 2. Add to GitHub Codespaces Secrets

Go to: https://github.com/settings/codespaces

Click "New secret" and add each of these:

| Secret Name | Description | Example Value |
|------------|-------------|---------------|
| `POSTGRES_PASSWORD` | PostgreSQL database password | `pTb0aOgFpHScHG4smO70MSLtPuLb4VrR` |
| `REDIS_PASSWORD` | Redis cache password | `5HrGspaUob4JcERDh1cnp2qUrVnJvGFV` |
| `PGADMIN_DEFAULT_PASSWORD` | PgAdmin web interface (optional) | `TeJvEKY7WsWvwm1gOAjS3x1aJCJepRLh` |
| `GRAFANA_ADMIN_PASSWORD` | Grafana monitoring (optional) | `vrFSFjFVXxplexdG5pr2u96mI7lryuct` |

### 3. Add API Keys (if you have them)

| Secret Name | Description | Where to Get |
|------------|-------------|--------------|
| `ALPHA_VANTAGE_API_KEY` | Stock market data | https://www.alphavantage.co/support/#api-key |
| `POLYGON_API_KEY` | Market data | https://polygon.io/ |
| `FINNHUB_API_KEY` | Financial data | https://finnhub.io/ |
| `FRED_API_KEY` | Federal Reserve data | https://fred.stlouisfed.org/docs/api/api_key.html |
| `NEWSAPI_KEY` | News sentiment | https://newsapi.org/ |

### 4. Repository vs User Secrets

You can set secrets at two levels:

**Repository Secrets** (Recommended for team projects):
- Go to your repository → Settings → Secrets and variables → Codespaces
- These are available only in Codespaces for this repository

**User Secrets** (For personal use across repos):
- Go to https://github.com/settings/codespaces
- These are available in all your Codespaces

## 🚀 Using Secrets in Codespaces

Once set, the secrets are automatically available as environment variables:

```bash
# Test that secrets are loaded
echo $POSTGRES_PASSWORD  # Should show [masked] or the actual value

# Use in your application
export DATABASE_URL="postgresql://neural_trader:$POSTGRES_PASSWORD@localhost:5432/neural_trader_db"
```

## 🛡️ Security Considerations

### DO:
- ✅ Use Codespaces Secrets for all sensitive data
- ✅ Generate different passwords for each environment
- ✅ Use long, random passwords (32+ characters)
- ✅ Rotate passwords periodically
- ✅ Use read-only database users where possible

### DON'T:
- ❌ Commit passwords to Git (even in .env files)
- ❌ Use the same password across environments
- ❌ Share Codespaces with secrets enabled
- ❌ Log or print passwords in your application
- ❌ Use simple or memorable passwords

## 🔧 Alternative: Using Docker Secrets

For production-like testing in Codespaces:

```yaml
# docker-compose.yml with secrets
services:
  timescaledb:
    environment:
      POSTGRES_PASSWORD_FILE: /run/secrets/postgres_password
    secrets:
      - postgres_password

secrets:
  postgres_password:
    file: ./secrets/postgres_password.txt
```

## 📝 Quick Test Script

After setting up secrets, test with:

```bash
#!/bin/bash
# Test that all secrets are available

echo "Testing Codespaces Secrets..."

check_secret() {
    if [ -z "${!1}" ]; then
        echo "❌ $1 is not set"
        return 1
    else
        echo "✅ $1 is set (length: ${#!1})"
        return 0
    fi
}

# Check required secrets
check_secret "POSTGRES_PASSWORD"
check_secret "REDIS_PASSWORD"

# Check optional secrets
check_secret "ALPHA_VANTAGE_API_KEY" || echo "   ℹ️  Optional - needed for market data"
check_secret "POLYGON_API_KEY" || echo "   ℹ️  Optional - needed for market data"
```

## 🔄 Updating Secrets

To update a secret:
1. Go to the Codespaces secrets page
2. Click on the secret name
3. Click "Update"
4. Enter the new value
5. Restart your Codespace for changes to take effect

## 🏷️ Environment-Specific Configurations

```bash
# Development (Codespaces)
POSTGRES_PASSWORD=${POSTGRES_PASSWORD:-$(openssl rand -base64 32)}

# Staging
POSTGRES_PASSWORD=${STAGING_POSTGRES_PASSWORD}

# Production
POSTGRES_PASSWORD=${PROD_POSTGRES_PASSWORD}  # From secrets manager
```