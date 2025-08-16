# Quick Start Guide: Data Backfill

## Overview

This guide will help you get started with the neural-trader Data Backfill system in under 10 minutes. Follow these steps to begin downloading historical market data from Polygon S3.

## Prerequisites

Before starting, ensure you have:

1. **Python 3.8+** installed
2. **Polygon API credentials** (access key and secret key)
3. **PostgreSQL with TimescaleDB** running
4. **Neural-trader** environment configured
5. **At least 100GB free disk space** for data storage

## Step 1: Environment Setup

### Set Environment Variables
```bash
# Required credentials
export POLYGON_ACCESS_KEY="your_polygon_api_key"
export POLYGON_SECRET_KEY="your_polygon_secret_key"

# Database configuration (if not already set)
export DATABASE_URL="postgresql://user:password@localhost:5432/neural_trader"

# Optional performance tuning
export BACKFILL_WORKERS=8
export BACKFILL_BATCH_SIZE=10000
```

### Verify Installation
```bash
# Check if backfill module is available
python -m data_ingestion.backfill --version

# Expected output:
# Neural-Trader Data Backfill v1.0.0
```

## Step 2: Basic Usage

### Download Recent Data (Last 7 Days)
```bash
# Download last 7 days for popular symbols
python -m data_ingestion.backfill \
    --symbols AAPL,MSFT,GOOGL,AMZN,TSLA \
    --days 7
```

### Download Specific Date Range
```bash
# Download Q1 2024 data
python -m data_ingestion.backfill \
    --symbols AAPL,MSFT \
    --start-date 2024-01-01 \
    --end-date 2024-03-31
```

### Download with Progress Display
```bash
# Enable detailed progress output
python -m data_ingestion.backfill \
    --symbols SPY \
    --days 30 \
    --progress \
    --verbose
```

## Step 3: Monitor Progress

### Real-time Progress
When running with `--progress`, you'll see:
```
📊 Backfill Progress
├── Downloading: 45/252 files (17.9%)
├── Processing: 38/45 files (84.4%)
├── Records: 1,234,567 inserted
├── Speed: 9,845 records/sec
├── ETA: 2h 15m remaining
└── Errors: 0
```

### Check Database
```sql
-- Connect to your database
psql $DATABASE_URL

-- Check imported data
SELECT 
    provider,
    date_trunc('day', time) as date,
    COUNT(*) as records,
    COUNT(DISTINCT symbol) as symbols
FROM market_data
WHERE provider = 'polygon_s3'
GROUP BY provider, date
ORDER BY date DESC
LIMIT 10;
```

## Step 4: Resume Interrupted Downloads

### Automatic Resume
```bash
# If download was interrupted, just run the same command
# It will automatically resume from checkpoint
python -m data_ingestion.backfill \
    --symbols AAPL,MSFT,GOOGL \
    --start-date 2024-01-01 \
    --end-date 2024-06-30
```

### Manual Resume with Checkpoint
```bash
# Resume from specific checkpoint file
python -m data_ingestion.backfill \
    --resume \
    --checkpoint-file /tmp/backfill_checkpoint_20240724.json
```

## Step 5: Verify Data Quality

### Run Validation
```bash
# Validate downloaded data
python -m data_ingestion.backfill \
    --validate \
    --symbols AAPL \
    --start-date 2024-07-01 \
    --end-date 2024-07-23
```

### Check for Gaps
```bash
# Find missing data
python -m data_ingestion.backfill \
    --check-gaps \
    --symbols AAPL,MSFT
```

## Common Use Cases

### 1. Initial Historical Load
```bash
# Download 1 year of data for analysis
python -m data_ingestion.backfill \
    --symbols-file symbols.txt \
    --start-date 2023-07-24 \
    --end-date 2024-07-23 \
    --workers 8
```

### 2. Daily Updates
```bash
# Add to cron for daily updates
# Runs at 7 AM ET after market data is available
0 7 * * * python -m data_ingestion.backfill --days 1 --symbols-file /path/to/symbols.txt
```

### 3. Selective Symbol Download
```bash
# Download only tech stocks
python -m data_ingestion.backfill \
    --symbols AAPL,MSFT,GOOGL,META,NVDA,AMD,INTC \
    --days 90 \
    --workers 4
```

### 4. Testing with Limited Data
```bash
# Test with small dataset
python -m data_ingestion.backfill \
    --symbols AAPL \
    --days 1 \
    --dry-run  # Shows what would be downloaded without executing
```

## Performance Tips

### 1. Optimize Worker Count
```bash
# For 8-core machine with good network
python -m data_ingestion.backfill \
    --symbols-file large_symbols.txt \
    --days 365 \
    --workers 8 \
    --batch-size 20000
```

### 2. Monitor Resource Usage
```bash
# Run with resource monitoring
python -m data_ingestion.backfill \
    --symbols SPY,QQQ \
    --days 30 \
    --monitor-resources \
    --max-memory 4G
```

### 3. Schedule During Off-Peak
```bash
# Limit bandwidth during business hours
python -m data_ingestion.backfill \
    --symbols-file symbols.txt \
    --days 7 \
    --bandwidth-limit 50  # MB/s
```

## Troubleshooting Quick Fixes

### Authentication Error
```bash
# Test credentials
python -m data_ingestion.backfill --test-connection

# If fails, check:
echo $POLYGON_ACCESS_KEY
echo $POLYGON_SECRET_KEY
```

### Database Connection Error
```bash
# Test database connection
python -m data_ingestion.backfill --test-db

# Check PostgreSQL is running
pg_isready -h localhost -p 5432
```

### Insufficient Disk Space
```bash
# Check available space
df -h

# Use compression flag
python -m data_ingestion.backfill \
    --symbols AAPL \
    --days 30 \
    --compress-downloads  # Deletes files after processing
```

### Memory Issues
```bash
# Reduce batch size and workers
python -m data_ingestion.backfill \
    --symbols-file symbols.txt \
    --days 7 \
    --workers 2 \
    --batch-size 5000 \
    --max-memory 2G
```

## Next Steps

### Explore Advanced Features
1. Read the [Configuration Guide](configuration-guide.md) for detailed options
2. Check the [API Reference](../api/python-api-reference.md) for programmatic usage
3. Review [Best Practices](best-practices.md) for production deployments

### Set Up Monitoring
1. Configure Grafana dashboards (see [Monitoring Guide](../maintenance/monitoring-guide.md))
2. Set up alerts for failed downloads
3. Track performance metrics

### Automate Workflows
1. Create scheduled jobs for regular updates
2. Build custom scripts for specific needs
3. Integrate with existing pipelines

## Quick Command Reference

```bash
# Help
python -m data_ingestion.backfill --help

# Version
python -m data_ingestion.backfill --version

# Test connections
python -m data_ingestion.backfill --test-all

# List available dates
python -m data_ingestion.backfill --list-available --year 2024 --month 7

# Dry run (show what would be done)
python -m data_ingestion.backfill --symbols AAPL --days 7 --dry-run

# Full verbose output
python -m data_ingestion.backfill --symbols MSFT --days 1 -vvv
```

## Getting Help

If you encounter issues:

1. Check the [Troubleshooting Guide](troubleshooting.md)
2. Review logs in `/var/log/neural-trader/backfill.log`
3. Run with `--debug` flag for detailed output
4. Contact support with error messages and logs

---

*Happy backfilling! 🚀*

*Document Version: 1.0.0 | Last Updated: July 2024*