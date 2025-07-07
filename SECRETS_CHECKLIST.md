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
