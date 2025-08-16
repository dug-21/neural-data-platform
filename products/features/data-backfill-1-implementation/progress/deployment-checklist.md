# Deployment Checklist

## Pre-Deployment Requirements

### Infrastructure Readiness

#### Storage
- [ ] Verify 2TB+ available space on data volume
- [ ] Mount external drives if using
- [ ] Set proper permissions (755 for directories, 644 for files)
- [ ] Configure backup destination
- [ ] Test write speeds (target: >100 MB/s)

#### Database
- [ ] TimescaleDB 2.11+ installed
- [ ] PostgreSQL 14+ running
- [ ] Connection pooling configured (pgBouncer optional)
- [ ] Hypertables created for market_data
- [ ] Compression policies configured
- [ ] Backup strategy in place

#### Network
- [ ] Stable internet connection (100+ Mbps recommended)
- [ ] AWS S3 connectivity verified
- [ ] Firewall rules configured
- [ ] DNS resolution working
- [ ] Proxy settings if applicable

#### System Resources
- [ ] CPU: 8+ cores available
- [ ] Memory: 16GB+ RAM
- [ ] Disk I/O: SSD recommended
- [ ] Swap: 8GB configured
- [ ] ulimits set appropriately

### Security Configuration

#### Credentials
- [ ] AWS credentials configured (profile: polygon-s3)
- [ ] Database credentials secured
- [ ] Redis password set (if using)
- [ ] API keys encrypted
- [ ] No hardcoded secrets in code

#### Access Control
- [ ] Database users created with minimal privileges
- [ ] File permissions properly set
- [ ] SELinux/AppArmor configured (if applicable)
- [ ] Network access restricted
- [ ] Audit logging enabled

#### Compliance
- [ ] Data retention policies defined
- [ ] GDPR compliance verified (if applicable)
- [ ] Security scan completed
- [ ] Vulnerability assessment passed
- [ ] Encryption at rest configured

### Software Dependencies

#### Python Environment
```bash
# Verify Python version
python --version  # Should be 3.8+

# Create virtual environment
python -m venv venv
source venv/bin/activate

# Install dependencies
pip install -r requirements.txt

# Verify key packages
pip show boto3 pandas asyncpg redis prometheus-client
```

#### System Packages
```bash
# Required system packages
sudo apt-get update
sudo apt-get install -y \
    python3-dev \
    postgresql-client \
    redis-tools \
    gzip \
    pigz \
    htop \
    iotop \
    sysstat
```

#### Docker (Optional)
```bash
# Docker version
docker --version  # Should be 20.10+
docker-compose --version  # Should be 1.29+

# Build images
docker build -t neural-trader/backfill .
docker-compose build
```

## Deployment Steps

### Step 1: Configuration Setup

#### Create Configuration File
```bash
# Create config directory
mkdir -p ~/.neural_trader

# Copy configuration template
cp config/backfill.yaml.template ~/.neural_trader/backfill.yaml

# Edit configuration
vim ~/.neural_trader/backfill.yaml
```

#### Set Environment Variables
```bash
# Create environment file
cat > ~/.neural_trader/backfill.env << EOF
# Database Configuration
DB_HOST=localhost
DB_PORT=5432
DB_NAME=trading
DB_USER=backfill_user
DB_PASSWORD=<secure_password>

# Redis Configuration
REDIS_HOST=localhost
REDIS_PORT=6379
REDIS_PASSWORD=<redis_password>

# AWS Configuration
AWS_PROFILE=polygon-s3

# Performance Settings
BACKFILL_WORKERS=10
BACKFILL_BATCH_SIZE=10000
BACKFILL_MEMORY_LIMIT=4096

# Logging
LOG_LEVEL=INFO
LOG_FILE=/var/log/neural_trader/backfill.log
EOF

# Load environment
source ~/.neural_trader/backfill.env
```

### Step 2: Database Preparation

#### Create Database Objects
```sql
-- Connect to database
psql -h localhost -U postgres -d trading

-- Create schema if needed
CREATE SCHEMA IF NOT EXISTS public;

-- Create backfill user
CREATE USER backfill_user WITH PASSWORD 'secure_password';
GRANT CONNECT ON DATABASE trading TO backfill_user;
GRANT USAGE ON SCHEMA public TO backfill_user;
GRANT CREATE ON SCHEMA public TO backfill_user;
GRANT INSERT, SELECT ON ALL TABLES IN SCHEMA public TO backfill_user;

-- Create market_data table if not exists
CREATE TABLE IF NOT EXISTS market_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    open NUMERIC(10,2) NOT NULL,
    high NUMERIC(10,2) NOT NULL,
    low NUMERIC(10,2) NOT NULL,
    close NUMERIC(10,2) NOT NULL,
    volume BIGINT NOT NULL,
    provider VARCHAR(50) NOT NULL,
    metadata JSONB,
    PRIMARY KEY (time, symbol)
);

-- Convert to hypertable
SELECT create_hypertable('market_data', 'time', if_not_exists => TRUE);

-- Add indexes
CREATE INDEX IF NOT EXISTS idx_market_data_symbol_time 
ON market_data (symbol, time DESC);

-- Set up compression
ALTER TABLE market_data SET (
    timescaledb.compress,
    timescaledb.compress_orderby = 'time DESC',
    timescaledb.compress_segmentby = 'symbol'
);

-- Add compression policy (after 7 days)
SELECT add_compression_policy('market_data', INTERVAL '7 days');
```

### Step 3: Initial Testing

#### Test S3 Access
```bash
# Test AWS credentials
aws s3 ls s3://flatfiles/us_stocks_sip/ --profile polygon-s3

# Test download
python scripts/download_polygon_s3.py \
    --profile polygon-s3 \
    --destination /tmp/test \
    --prefix us_stocks_sip/day_aggs_v1/2024/01/ \
    --max-files 1 \
    --dry-run
```

#### Test Database Connection
```bash
# Test with psql
psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "SELECT version();"

# Test with Python
python -c "
import asyncio
import asyncpg

async def test():
    conn = await asyncpg.connect(
        host='$DB_HOST',
        port=$DB_PORT,
        user='$DB_USER',
        password='$DB_PASSWORD',
        database='$DB_NAME'
    )
    version = await conn.fetchval('SELECT version()')
    print(f'Connected: {version}')
    await conn.close()

asyncio.run(test())
"
```

#### Test CLI
```bash
# Show help
python -m data_ingestion.backfill --help

# Run diagnostic
python -m data_ingestion.backfill diagnose

# Test with small dataset
python -m data_ingestion.backfill file \
    --path /tmp/test \
    --format csv \
    --dry-run
```

### Step 4: Monitoring Setup

#### Start Metrics Server
```bash
# Run metrics exporter
python -m data_ingestion.backfill.metrics_server --port 8000 &

# Verify metrics
curl http://localhost:8000/metrics | grep backfill
```

#### Configure Prometheus
```yaml
# /etc/prometheus/prometheus.yml
scrape_configs:
  - job_name: 'backfill'
    static_configs:
      - targets: ['localhost:8000']
    scrape_interval: 15s
```

#### Import Grafana Dashboard
```bash
# Import dashboard
curl -X POST http://admin:admin@localhost:3000/api/dashboards/db \
    -H "Content-Type: application/json" \
    -d @monitoring/grafana-dashboard.json
```

### Step 5: Production Deployment

#### Create Systemd Service
```ini
# /etc/systemd/system/neural-trader-backfill.service
[Unit]
Description=Neural Trader Data Backfill Service
After=network.target postgresql.service

[Service]
Type=simple
User=backfill
Group=backfill
WorkingDirectory=/opt/neural-trader
Environment="PATH=/opt/neural-trader/venv/bin:/usr/local/bin:/usr/bin:/bin"
EnvironmentFile=/opt/neural-trader/.env
ExecStart=/opt/neural-trader/venv/bin/python -m data_ingestion.backfill.service
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

#### Enable Service
```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable neural-trader-backfill

# Start service
sudo systemctl start neural-trader-backfill

# Check status
sudo systemctl status neural-trader-backfill
```

## Validation Steps

### Functional Validation

- [ ] Small test backfill completes successfully
- [ ] Data appears in database correctly
- [ ] Checkpoints are saved and can resume
- [ ] Error handling works as expected
- [ ] Monitoring metrics are collected

### Performance Validation

- [ ] Processing rate meets target (10K+ records/sec)
- [ ] Memory usage stays within limits (<4GB)
- [ ] CPU usage is reasonable (<80%)
- [ ] Network bandwidth is utilized efficiently
- [ ] Database inserts are performant

### Data Quality Validation

```sql
-- Check record counts
SELECT 
    DATE(time) as date,
    COUNT(*) as records,
    COUNT(DISTINCT symbol) as symbols
FROM market_data
WHERE provider = 'polygon_s3'
GROUP BY DATE(time)
ORDER BY date DESC
LIMIT 10;

-- Check for gaps
WITH expected_minutes AS (
    SELECT generate_series(
        '2024-01-01 09:30:00'::timestamp,
        '2024-01-01 16:00:00'::timestamp,
        '1 minute'::interval
    ) AS minute
)
SELECT COUNT(*) as missing_minutes
FROM expected_minutes e
LEFT JOIN market_data m ON DATE_TRUNC('minute', m.time) = e.minute
    AND m.symbol = 'AAPL'
WHERE m.time IS NULL;

-- Validate OHLC consistency
SELECT COUNT(*) as invalid_records
FROM market_data
WHERE high < low 
   OR high < open 
   OR high < close
   OR low > open
   OR low > close;
```

## Rollback Plan

### Immediate Rollback

```bash
# Stop service
sudo systemctl stop neural-trader-backfill

# Remove bad data (if needed)
psql -h $DB_HOST -U $DB_USER -d $DB_NAME << EOF
DELETE FROM market_data 
WHERE provider = 'file_import' 
AND time >= '2024-01-01';
EOF

# Clear checkpoints
redis-cli --no-auth-warning -a $REDIS_PASSWORD FLUSHDB

# Restore from backup
pg_restore -h $DB_HOST -U $DB_USER -d $DB_NAME backup_before_deployment.dump
```

### Checkpoint Recovery

```bash
# List checkpoints
python -m data_ingestion.backfill status --show-checkpoints

# Resume from specific checkpoint
python -m data_ingestion.backfill resume \
    --operation-id op_20240724_123456 \
    --force
```

## Post-Deployment

### Monitoring Checklist

- [ ] All services running (check systemctl)
- [ ] Metrics being collected (check Prometheus)
- [ ] Dashboards showing data (check Grafana)
- [ ] Logs rotating properly (check logrotate)
- [ ] Alerts configured and working

### Performance Tuning

```bash
# Monitor real-time performance
htop
iotop -o
nethogs

# Database connections
watch -n 1 "psql -h $DB_HOST -U $DB_USER -d $DB_NAME -c 'SELECT count(*) FROM pg_stat_activity;'"

# Disk usage
df -h /mnt/data
du -sh /mnt/data/polygon_data/
```

### Documentation Updates

- [ ] Update runbook with actual commands used
- [ ] Document any deviations from plan
- [ ] Record performance metrics achieved
- [ ] Note any issues encountered
- [ ] Update contact information

## Sign-offs

### Pre-Deployment

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Dev Lead | | | |
| QA Lead | | | |
| DBA | | | |
| Security | | | |
| Operations | | | |

### Post-Deployment

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Dev Lead | | | |
| Operations | | | |
| Product Owner | | | |

## Emergency Contacts

- **On-Call Engineer**: +1-XXX-XXX-XXXX
- **Database Admin**: +1-XXX-XXX-XXXX
- **AWS Support**: [AWS Console](https://console.aws.amazon.com/support)
- **Escalation**: engineering-leads@company.com

---

*Version: 1.0.0*  
*Last Updated: July 24, 2024*  
*Next Review: August 24, 2024*