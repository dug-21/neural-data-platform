# Complete Data Backfill Process for Neural Trader using Polygon Flatfiles

## Executive Summary

This document provides a comprehensive guide for backfilling 5 years of minute aggregate data from Polygon's S3 flatfiles. The neural-trader codebase has robust infrastructure supporting parallel downloads, checkpoint/resume functionality, and high-performance data ingestion at 11,000+ records/second.

## Current Capabilities

### Infrastructure Components
- **Download Script**: `scripts/download_polygon_s3.py` - Standalone S3 downloader with checkpoint/resume
- **CLI Interface**: `data_ingestion/cli/backfill.py` - Full-featured CLI with multiple commands
- **File Provider**: `data_ingestion/providers/file_provider.py` - Processes downloaded files
- **Storage**: TimescaleDB with hypertables optimized for time-series data
- **Performance**: Validated at 11,000+ records/second ingestion rate

### Key Features
- ✅ Parallel S3 downloads (10 concurrent streams)
- ✅ Checkpoint/resume for interrupted downloads
- ✅ External drive support for large datasets
- ✅ OHLC validation with consistency checks
- ✅ Automatic retry with exponential backoff
- ✅ Progress tracking and monitoring
- ✅ Docker containerization for isolation

## Prerequisites

### 1. System Requirements
- **Storage**: Minimum 1.5TB free space (for 5 years of data)
- **Memory**: 8GB RAM minimum, 16GB recommended
- **CPU**: 4+ cores for optimal parallel processing
- **Network**: Stable broadband connection (50+ Mbps recommended)

### 2. Polygon Credentials
```bash
# Get from Polygon.io dashboard and set as environment variables
export AWS_ACCESS_KEY_ID="your_polygon_access_key"
export AWS_SECRET_ACCESS_KEY="your_polygon_secret_key"
```

### 3. AWS Profile Setup (Optional)
If you prefer using AWS profiles instead of environment variables:
```bash
# Configure AWS CLI with Polygon credentials
aws configure --profile polygon-s3
# AWS Access Key ID: [Your Polygon Access Key]
# AWS Secret Access Key: [Your Polygon Secret Key]
# Default region: us-east-1
# Default output: json
```

## Process Overview

### Phase 1: Environment Setup

1. **Start Docker Services**
```bash
# Navigate to project root
cd /workspaces/neural-trader

# Start TimescaleDB and Redis
docker-compose up -d timescaledb redis

# Verify services are running
docker-compose ps
```

2. **Prepare External Storage**
```bash
# Create download directory on external drive
sudo mkdir -p /mnt/external/polygon-data
sudo chown $USER:$USER /mnt/external/polygon-data

# Verify space available
df -h /mnt/external
```

### Phase 2: Initial Data Download

#### Option A: Using Standalone Download Script (Recommended for Large Backfills)

```bash
# Install dependencies
pip install -r scripts/requirements_polygon_download.txt

# Download 5 years of minute aggregates (using environment variables)
python3 scripts/download_polygon_s3.py \
    --destination /mnt/external/polygon-data \
    --prefix us_stocks_sip/minute_aggs_v1/ \
    --start-date 2019-01-01 \
    --end-date 2024-01-01 \
    --log-file polygon_download.log

# Or if using AWS profile:
# python3 scripts/download_polygon_s3.py \
#     --profile polygon-s3 \
#     --destination /mnt/external/polygon-data \
#     --prefix us_stocks_sip/minute_aggs_v1/ \
#     --start-date 2019-01-01 \
#     --end-date 2024-01-01 \
#     --log-file polygon_download.log
```

#### Option B: Using Docker-based CLI

```bash
# Build the data ingestion container
docker build -f docker/data-ingestion/Dockerfile -t neural-trader-backfill .

# Run backfill from container
docker run -it --rm \
    --network neural-trader_neural_trader_net \
    -v /mnt/external/polygon-data:/data \
    -e POLYGON_ACCESS_KEY=$POLYGON_ACCESS_KEY \
    -e POLYGON_SECRET_KEY=$POLYGON_SECRET_KEY \
    neural-trader-backfill \
    python -m data_ingestion.cli.backfill s3 \
        --profile polygon-s3 \
        --symbols ALL \
        --start-date 2019-01-01 \
        --end-date 2024-01-01 \
        --destination /data \
        --max-workers 10
```

### Phase 3: Process Downloaded Files

1. **Import Data into TimescaleDB**
```bash
# Process all downloaded files
docker run -it --rm \
    --network neural-trader_neural_trader_net \
    -v /mnt/external/polygon-data:/data \
    neural-trader-backfill \
    python -m data_ingestion.cli.backfill file \
        --path /data/polygon_data \
        --format csv \
        --recursive \
        --batch-size 10000 \
        --workers 8 \
        --checkpoint
```

2. **Monitor Progress**
```bash
# In another terminal, check status
docker run -it --rm \
    --network neural-trader_neural_trader_net \
    neural-trader-backfill \
    python -m data_ingestion.cli.backfill status --detailed
```

### Phase 4: Data Validation

1. **Run Validation Checks**
```bash
# Validate data completeness
docker run -it --rm \
    --network neural-trader_neural_trader_net \
    neural-trader-backfill \
    python -m data_ingestion.cli.backfill validate \
        --symbols AAPL,MSFT,GOOGL,AMZN,TSLA \
        --start-date 2019-01-01 \
        --end-date 2024-01-01 \
        --checks all
```

2. **Check for Gaps**
```sql
-- Connect to TimescaleDB
docker exec -it neural_trader_timescaledb psql -U neural_trader -d neural_trader_db

-- Check data coverage
SELECT 
    date_trunc('month', time) as month,
    COUNT(DISTINCT date_trunc('day', time)) as trading_days,
    COUNT(DISTINCT symbol) as symbols,
    COUNT(*) as total_records
FROM market_data
WHERE provider = 'polygon_s3'
GROUP BY month
ORDER BY month;

-- Find gaps in specific symbols
SELECT 
    symbol,
    date_trunc('day', time) as date,
    COUNT(*) as minute_bars
FROM market_data
WHERE symbol IN ('AAPL', 'MSFT', 'GOOGL')
    AND time >= '2023-01-01'
    AND time < '2024-01-01'
GROUP BY symbol, date
HAVING COUNT(*) < 390  -- Less than full trading day
ORDER BY symbol, date;
```

## Detailed Workflow Steps

### Step 1: Prepare Symbol List

Create a file with symbols to download (optional, downloads all by default):
```bash
# Create symbols file
cat > /mnt/external/symbols.txt << EOF
AAPL
MSFT
GOOGL
AMZN
TSLA
META
NVDA
# Add more symbols as needed
EOF
```

### Step 2: Download in Batches (Recommended)

For 5 years of data, download in yearly batches to manage disk space:

```bash
#!/bin/bash
# download_by_year.sh

YEARS=(2019 2020 2021 2022 2023)
DESTINATION="/mnt/external/polygon-data"

for YEAR in "${YEARS[@]}"; do
    echo "Downloading data for year: $YEAR"
    
    python3 scripts/download_polygon_s3.py \
        --destination $DESTINATION \
        --prefix us_stocks_sip/minute_aggs_v1/ \
        --start-date "${YEAR}-01-01" \
        --end-date "${YEAR}-12-31" \
        --log-file "polygon_download_${YEAR}.log"
    
    # Process the year's data
    echo "Processing data for year: $YEAR"
    docker run -it --rm \
        --network neural-trader_neural_trader_net \
        -v $DESTINATION:/data \
        neural-trader-backfill \
        python -m data_ingestion.cli.backfill file \
            --path /data/polygon_data/${YEAR} \
            --format csv \
            --recursive \
            --batch-size 10000 \
            --workers 8
    
    # Optional: Archive processed files to save space
    # tar -czf "${DESTINATION}/archive/${YEAR}.tar.gz" "${DESTINATION}/polygon_data/${YEAR}"
    # rm -rf "${DESTINATION}/polygon_data/${YEAR}"
done
```

### Step 3: Handle Interruptions

If download is interrupted:
```bash
# Simply re-run the same command - it will resume from checkpoint
python3 scripts/download_polygon_s3.py \
    --profile polygon-s3 \
    --destination /mnt/external/polygon-data \
    --prefix us_stocks_sip/minute_aggs_v1/ \
    --start-date 2019-01-01 \
    --end-date 2024-01-01

# Or retry failed downloads specifically
python3 scripts/download_polygon_s3.py \
    --profile polygon-s3 \
    --destination /mnt/external/polygon-data \
    --retry-failed
```

### Step 4: Monitor System Resources

```bash
# Monitor disk usage
watch -n 10 'df -h /mnt/external'

# Monitor processing speed
docker logs -f neural-trader-backfill

# Check database size
docker exec -it neural_trader_timescaledb psql -U neural_trader -d neural_trader_db -c \
    "SELECT pg_size_pretty(pg_database_size('neural_trader_db'));"
```

## Performance Optimization

### 1. Parallel Processing Configuration
```bash
# Optimal settings for 8-core machine with 16GB RAM
export BACKFILL_WORKERS=8
export BACKFILL_BATCH_SIZE=20000
export POSTGRES_MAX_CONNECTIONS=200
```

### 2. TimescaleDB Tuning
```sql
-- Optimize for bulk inserts
ALTER SYSTEM SET max_wal_size = '4GB';
ALTER SYSTEM SET checkpoint_timeout = '30min';
ALTER SYSTEM SET synchronous_commit = 'off';

-- Reload configuration
SELECT pg_reload_conf();
```

### 3. Storage Optimization
```sql
-- Enable compression on older data
SELECT add_compression_policy('market_data', INTERVAL '30 days');

-- Create continuous aggregates for common queries
CREATE MATERIALIZED VIEW market_data_daily
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 day', time) AS day,
    symbol,
    first(open, time) AS open,
    max(high) AS high,
    min(low) AS low,
    last(close, time) AS close,
    sum(volume) AS volume
FROM market_data
GROUP BY day, symbol;
```

## Troubleshooting

### Common Issues and Solutions

1. **AWS Authentication Errors**
```bash
# Test S3 access
aws s3 ls s3://flatfiles/ --endpoint-url https://files.polygon.io --profile polygon-s3

# If fails, verify credentials
aws configure list --profile polygon-s3
```

2. **Insufficient Disk Space**
```bash
# Check space before starting
df -h /mnt/external

# Clean up completed files after processing
find /mnt/external/polygon-data -name "*.csv.gz" -mtime +7 -delete
```

3. **Database Connection Issues**
```bash
# Check TimescaleDB is running
docker-compose ps timescaledb

# Test connection
docker exec -it neural_trader_timescaledb pg_isready

# View logs
docker-compose logs -f timescaledb
```

4. **Memory Issues During Processing**
```bash
# Reduce batch size and workers
python -m data_ingestion.cli.backfill file \
    --path /data \
    --batch-size 5000 \
    --workers 4
```

## Maintenance Tasks

### Daily Updates (After Initial Backfill)
```bash
# Add to crontab for daily updates at 7 AM ET
0 7 * * * /path/to/daily_update.sh

# daily_update.sh content:
#!/bin/bash
YESTERDAY=$(date -d "yesterday" +%Y-%m-%d)

python3 /path/to/scripts/download_polygon_s3.py \
    --profile polygon-s3 \
    --destination /mnt/external/polygon-data \
    --start-date $YESTERDAY \
    --end-date $YESTERDAY

docker run --rm \
    --network neural-trader_neural_trader_net \
    -v /mnt/external/polygon-data:/data \
    neural-trader-backfill \
    python -m data_ingestion.cli.backfill file \
        --path /data/polygon_data \
        --start-date $YESTERDAY
```

### Data Validation Reports
```bash
# Weekly validation report
docker run --rm \
    --network neural-trader_neural_trader_net \
    neural-trader-backfill \
    python -m data_ingestion.cli.backfill validate \
        --symbols ALL \
        --start-date $(date -d "7 days ago" +%Y-%m-%d) \
        --end-date $(date +%Y-%m-%d) \
        --report /tmp/weekly_validation.json
```

## Expected Results

### Storage Requirements
- **Raw Downloads**: ~250GB compressed (5 years)
- **Database Size**: ~1.2TB uncompressed in TimescaleDB
- **Processing Time**: ~48-72 hours for full 5-year backfill

### Performance Metrics
- **Download Speed**: 50-85 MB/s (depends on connection)
- **Processing Rate**: 11,000+ records/second
- **Daily Updates**: ~15-30 minutes for previous day

## Conclusion

The neural-trader system is fully equipped to handle large-scale data backfills from Polygon flatfiles. The combination of checkpoint/resume functionality, parallel processing, and optimized storage ensures reliable and efficient data ingestion. Follow this process to successfully backfill 5 years of minute aggregate data for your trading strategies.

## Support Resources

- **Logs**: Check `/var/log/neural-trader/backfill.log`
- **Metrics**: Monitor via Prometheus/Grafana dashboards
- **Database**: Use pgAdmin or psql for direct queries
- **Documentation**: See `/products/features/data-backfill-implementation/` for detailed guides

---
*Document Version: 1.0.0 | Created: January 2025*