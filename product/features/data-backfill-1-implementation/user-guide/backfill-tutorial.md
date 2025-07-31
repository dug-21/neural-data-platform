# Backfill Tutorial

## Overview

This step-by-step tutorial will guide you through the complete process of backfilling historical market data from Polygon's S3 storage into your TimescaleDB database.

## Prerequisites

Before starting, ensure you have:

1. ✅ AWS account with Polygon S3 access
2. ✅ TimescaleDB instance running
3. ✅ Sufficient disk space (estimate: 1GB per symbol per year)
4. ✅ Python 3.8+ installed
5. ✅ Docker and Docker Compose (optional)

## Table of Contents

1. [Initial Setup](#initial-setup)
2. [Small-Scale Test](#small-scale-test)
3. [Production Backfill](#production-backfill)
4. [Monitoring Progress](#monitoring-progress)
5. [Validation](#validation)
6. [Troubleshooting](#troubleshooting)

## Initial Setup

### Step 1: Clone Repository

```bash
# Clone the neural-trader repository
git clone https://github.com/your-org/neural-trader.git
cd neural-trader

# Navigate to data ingestion module
cd data_ingestion
```

### Step 2: Install Dependencies

```bash
# Create virtual environment (recommended)
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install required packages
pip install -r requirements.txt

# Install additional packages for S3 support
pip install boto3 pyarrow tqdm
```

### Step 3: Configure AWS Credentials

```bash
# Configure AWS profile for Polygon S3 access
aws configure --profile polygon-s3

# Enter your credentials when prompted:
AWS Access Key ID [None]: YOUR_ACCESS_KEY
AWS Secret Access Key [None]: YOUR_SECRET_KEY
Default region name [None]: us-east-1
Default output format [None]: json

# Verify access
aws s3 ls s3://flatfiles/us_stocks_sip/ --profile polygon-s3
```

### Step 4: Set Up Configuration

Create configuration file `~/.neural_trader/backfill.yaml`:

```yaml
backfill:
  defaults:
    batch_size: 10000
    workers: 5
    checkpoint: true

s3:
  profile: polygon-s3
  bucket: flatfiles
  download_path: /mnt/data/polygon

database:
  host: localhost
  port: 5432
  name: trading
  username: ${DB_USER}
  password: ${DB_PASSWORD}

redis:
  host: localhost
  port: 6379
```

Set environment variables:

```bash
export DB_USER=your_db_user
export DB_PASSWORD=your_db_password
export NEURAL_TRADER_CONFIG=~/.neural_trader/backfill.yaml
```

## Small-Scale Test

Before running a full backfill, test with a small dataset.

### Step 1: Test S3 Download

```bash
# Test downloading one day of data for one symbol
python scripts/download_polygon_s3.py \
  --profile polygon-s3 \
  --destination /tmp/test_download \
  --prefix us_stocks_sip/day_aggs_v1/2023/01/ \
  --pattern "2023-01-03" \
  --max-files 1

# Check downloaded file
ls -la /tmp/test_download/polygon_data/2023/01/
```

### Step 2: Test File Import

```bash
# Import the downloaded test file
python -m data_ingestion.backfill file \
  --path /tmp/test_download/polygon_data \
  --format csv \
  --dry-run

# If dry-run looks good, run actual import
python -m data_ingestion.backfill file \
  --path /tmp/test_download/polygon_data \
  --format csv
```

### Step 3: Verify Data

```sql
-- Connect to database and check imported data
psql -h localhost -U your_db_user -d trading

-- Check record count
SELECT COUNT(*) FROM market_data 
WHERE time >= '2023-01-03' AND time < '2023-01-04';

-- Sample records
SELECT * FROM market_data 
WHERE time >= '2023-01-03' AND time < '2023-01-04'
LIMIT 10;
```

## Production Backfill

Once testing is successful, proceed with full backfill.

### Step 1: Plan Your Backfill

Calculate requirements:

```python
# Estimate data size and time
symbols = 100  # Number of symbols
years = 5      # Years of history
gb_per_symbol_year = 1

total_size_gb = symbols * years * gb_per_symbol_year
print(f"Estimated size: {total_size_gb} GB")

# At 50 MB/s download speed
download_time_hours = (total_size_gb * 1024) / (50 * 3600)
print(f"Estimated download time: {download_time_hours:.1f} hours")
```

### Step 2: Create Symbol List

```bash
# Create a file with symbols to backfill
cat > symbols.txt << EOF
AAPL
MSFT
GOOGL
AMZN
META
TSLA
NVDA
JPM
V
JNJ
EOF
```

### Step 3: Start S3 Download

```bash
# Download data for multiple symbols
SYMBOLS=$(cat symbols.txt | tr '\n' ',' | sed 's/,$//')

python scripts/download_polygon_s3.py \
  --profile polygon-s3 \
  --destination /mnt/external/polygon_data \
  --prefix us_stocks_sip/day_aggs_v1/ \
  --start-date 2019-01-01 \
  --end-date 2023-12-31 \
  --pattern "$SYMBOLS" \
  --log-file polygon_download.log &

# Monitor progress
tail -f /mnt/external/polygon_data/polygon_download.log
```

### Step 4: Import Downloaded Data

Run import in parallel with download:

```bash
# Start import process (in another terminal)
python -m data_ingestion.backfill file \
  --path /mnt/external/polygon_data \
  --format csv \
  --recursive \
  --symbols-file symbols.txt \
  --start-date 2019-01-01 \
  --end-date 2023-12-31 \
  --workers 10 \
  --batch-size 20000 &

# Save process ID
echo $! > backfill.pid
```

### Step 5: Using Docker (Alternative)

```bash
# Build Docker image
docker build -t neural-trader/backfill .

# Run with Docker Compose
docker-compose -f docker-compose.backfill.yml up -d

# Monitor logs
docker-compose -f docker-compose.backfill.yml logs -f backfill
```

Example `docker-compose.backfill.yml`:

```yaml
version: '3.8'

services:
  backfill:
    image: neural-trader/backfill
    environment:
      - AWS_PROFILE=polygon-s3
      - DB_HOST=timescale
      - DB_USER=${DB_USER}
      - DB_PASSWORD=${DB_PASSWORD}
      - REDIS_HOST=redis
    volumes:
      - ~/.aws:/root/.aws:ro
      - /mnt/external:/mnt/external
      - ./logs:/var/log/neural_trader
    command: >
      python -m data_ingestion.backfill file
      --path /mnt/external/polygon_data
      --format csv
      --recursive
      --workers 10
    restart: on-failure
    
  timescale:
    image: timescale/timescaledb:latest-pg14
    environment:
      - POSTGRES_DB=trading
      - POSTGRES_USER=${DB_USER}
      - POSTGRES_PASSWORD=${DB_PASSWORD}
    volumes:
      - timescale_data:/var/lib/postgresql/data
      
  redis:
    image: redis:alpine
    volumes:
      - redis_data:/data

volumes:
  timescale_data:
  redis_data:
```

## Monitoring Progress

### Step 1: Check Operation Status

```bash
# View current status
python -m data_ingestion.backfill status

# Example output:
# Operation ID: op_2023-07-24_abc123
# Status: RUNNING
# Progress: 45.5% (1,234,567 / 2,711,234 records)
# Files: 456 / 1,825 completed
# Speed: 11,234 records/sec
# ETA: 2023-07-24 15:30:00
```

### Step 2: Monitor System Resources

```bash
# CPU and Memory
htop

# Disk I/O
iotop

# Network usage
iftop

# Database connections
psql -c "SELECT count(*) FROM pg_stat_activity WHERE application_name LIKE '%backfill%';"
```

### Step 3: View Metrics Dashboard

If metrics are enabled:

```bash
# Access Prometheus metrics
curl http://localhost:8000/metrics | grep backfill

# Key metrics to monitor:
# - backfill_records_processed_total
# - backfill_processing_rate_rps
# - backfill_errors_total
# - backfill_checkpoint_saves_total
```

### Step 4: Check Logs

```bash
# Application logs
tail -f /var/log/neural_trader/backfill.log

# Filter for errors
grep ERROR /var/log/neural_trader/backfill.log

# Database logs
tail -f /var/log/postgresql/postgresql-*.log
```

## Validation

After backfill completes, validate the data.

### Step 1: Run Validation Checks

```bash
# Run comprehensive validation
python -m data_ingestion.backfill validate \
  --symbols-file symbols.txt \
  --start-date 2019-01-01 \
  --end-date 2023-12-31 \
  --checks all \
  --report validation_report.html
```

### Step 2: Check Data Completeness

```sql
-- Check daily record counts
SELECT 
    date_trunc('day', time) as day,
    symbol,
    COUNT(*) as records
FROM market_data
WHERE time >= '2019-01-01'
GROUP BY day, symbol
ORDER BY day, symbol;

-- Find gaps
WITH expected_minutes AS (
    SELECT generate_series(
        '2023-01-03 09:30:00'::timestamp,
        '2023-01-03 16:00:00'::timestamp,
        '1 minute'::interval
    ) AS minute
)
SELECT e.minute
FROM expected_minutes e
LEFT JOIN market_data m ON date_trunc('minute', m.time) = e.minute
    AND m.symbol = 'AAPL'
WHERE m.time IS NULL
ORDER BY e.minute;
```

### Step 3: Verify Data Quality

```sql
-- Check for OHLC consistency
SELECT symbol, date_trunc('day', time) as day, COUNT(*) as violations
FROM market_data
WHERE high < low 
   OR high < open 
   OR high < close
   OR low > open
   OR low > close
GROUP BY symbol, day;

-- Check for duplicates
SELECT symbol, time, COUNT(*) as count
FROM market_data
GROUP BY symbol, time
HAVING COUNT(*) > 1;
```

## Troubleshooting

### Common Issues and Solutions

#### 1. Slow Download Speed

**Symptom**: Downloads are slower than expected

**Solutions**:
```bash
# Increase concurrent downloads
python scripts/download_polygon_s3.py \
  --max-concurrent 20 \
  ...

# Check network bandwidth
speedtest-cli

# Use different S3 endpoint if available
export S3_ENDPOINT_URL=https://s3-accelerate.amazonaws.com
```

#### 2. Database Connection Errors

**Symptom**: "connection pool exhausted" errors

**Solutions**:
```bash
# Increase connection pool size
export DB_POOL_MAX_SIZE=50

# Or in config file
database:
  pool:
    max_size: 50

# Check current connections
psql -c "SELECT count(*) FROM pg_stat_activity;"
```

#### 3. Memory Issues

**Symptom**: Process killed or OOM errors

**Solutions**:
```bash
# Reduce batch size
python -m data_ingestion.backfill file \
  --batch-size 5000 \
  ...

# Limit memory usage
export BACKFILL_MEMORY_LIMIT=1024  # MB

# Monitor memory
watch -n 1 free -h
```

#### 4. Checkpoint Recovery

**Symptom**: Need to resume after failure

**Solutions**:
```bash
# List available checkpoints
python -m data_ingestion.backfill status --show-checkpoints

# Resume from checkpoint
python -m data_ingestion.backfill resume \
  --operation-id op_2023-07-24_abc123

# Force resume if checkpoint is corrupted
python -m data_ingestion.backfill resume \
  --operation-id op_2023-07-24_abc123 \
  --force
```

#### 5. Data Validation Failures

**Symptom**: High percentage of bad records

**Solutions**:
```bash
# Check file format
file /mnt/external/polygon_data/2023/01/01/data.csv.gz
zcat /mnt/external/polygon_data/2023/01/01/data.csv.gz | head

# Run with relaxed validation
python -m data_ingestion.backfill file \
  --path /mnt/external/polygon_data \
  --no-strict-validation \
  ...

# Skip bad files
python -m data_ingestion.backfill file \
  --skip-errors \
  ...
```

### Getting Help

If you encounter issues:

1. Check logs for detailed error messages
2. Run validation to identify data issues
3. Use `--debug` flag for verbose output
4. Consult the troubleshooting guide
5. Search existing issues on GitHub
6. Create a new issue with:
   - Error messages
   - Configuration used
   - Sample of problematic data
   - System specifications

## Best Practices

1. **Start Small**: Test with one symbol for one day
2. **Monitor Progress**: Keep terminals open with logs
3. **Use Checkpoints**: Enable for long-running backfills
4. **Validate Often**: Run validation after each major batch
5. **Plan Downtime**: Some operations may impact database performance
6. **Document Process**: Keep notes on settings that work
7. **Backup First**: Always backup database before major operations

## Next Steps

After successful backfill:

1. Set up continuous data ingestion
2. Create data quality monitoring
3. Build aggregation views
4. Implement data retention policies
5. Schedule regular validation
6. Document your setup for team

Congratulations! You've successfully backfilled historical market data.