# Backfill CLI API Reference

## Overview

The backfill CLI provides command-line interface for managing historical data imports from various sources including Polygon S3 and local files.

## Installation

```bash
# Install the data ingestion module
cd /workspaces/neural-trader/data_ingestion
pip install -r requirements.txt
```

## Basic Usage

```bash
# Basic command structure
python -m data_ingestion.backfill [command] [options]

# Or using the standalone script
python scripts/run_backfill.py [options]
```

## Commands

### `backfill s3`

Download and import data from Polygon S3 storage.

```bash
python -m data_ingestion.backfill s3 \
  --profile polygon-s3 \
  --symbols AAPL,MSFT,GOOGL \
  --start-date 2023-01-01 \
  --end-date 2023-12-31 \
  --data-type day_aggs_v1
```

#### Options

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--profile` | AWS profile name | None | Yes |
| `--symbols` | Comma-separated symbols | None | Yes |
| `--start-date` | Start date (YYYY-MM-DD) | None | Yes |
| `--end-date` | End date (YYYY-MM-DD) | None | Yes |
| `--data-type` | Data type to download | day_aggs_v1 | No |
| `--destination` | Local storage path | /mnt/data | No |
| `--max-workers` | Parallel download workers | 10 | No |
| `--batch-size` | Records per batch | 10000 | No |
| `--checkpoint` | Enable checkpointing | True | No |
| `--dry-run` | Preview without downloading | False | No |

### `backfill file`

Import data from local files.

```bash
python -m data_ingestion.backfill file \
  --path /mnt/external/polygon_data \
  --format csv \
  --symbols AAPL,MSFT \
  --recursive
```

#### Options

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--path` | File or directory path | None | Yes |
| `--format` | File format (csv, json, parquet) | csv | No |
| `--symbols` | Filter by symbols | None | No |
| `--start-date` | Start date filter | None | No |
| `--end-date` | End date filter | None | No |
| `--recursive` | Search subdirectories | False | No |
| `--pattern` | File name pattern | *.{format} | No |
| `--batch-size` | Records per batch | 10000 | No |
| `--checkpoint` | Enable checkpointing | True | No |
| `--dry-run` | Preview without importing | False | No |

### `backfill status`

Check backfill operation status.

```bash
python -m data_ingestion.backfill status \
  --operation-id abc123 \
  --detailed
```

#### Options

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--operation-id` | Specific operation ID | None | No |
| `--detailed` | Show detailed progress | False | No |
| `--format` | Output format (json, table) | table | No |

### `backfill resume`

Resume a interrupted backfill operation.

```bash
python -m data_ingestion.backfill resume \
  --operation-id abc123 \
  --force
```

#### Options

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--operation-id` | Operation to resume | None | Yes |
| `--force` | Force resume despite warnings | False | No |
| `--skip-validation` | Skip checkpoint validation | False | No |

### `backfill validate`

Validate imported data quality.

```bash
python -m data_ingestion.backfill validate \
  --symbols AAPL,MSFT \
  --start-date 2023-01-01 \
  --end-date 2023-12-31 \
  --checks all
```

#### Options

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--symbols` | Symbols to validate | None | Yes |
| `--start-date` | Start date | None | Yes |
| `--end-date` | End date | None | Yes |
| `--checks` | Validation checks to run | all | No |
| `--fix` | Attempt to fix issues | False | No |
| `--report` | Generate validation report | False | No |

## Configuration

### Environment Variables

```bash
# S3 Configuration
export AWS_PROFILE=polygon-s3
export AWS_DEFAULT_REGION=us-east-1

# Database Configuration
export DB_HOST=localhost
export DB_PORT=5432
export DB_NAME=trading
export DB_USER=backfill_user
export DB_PASSWORD=secure_password

# Redis Configuration (for checkpoints)
export REDIS_HOST=localhost
export REDIS_PORT=6379
export REDIS_DB=0

# Performance Tuning
export BACKFILL_WORKERS=10
export BACKFILL_BATCH_SIZE=10000
export BACKFILL_MEMORY_LIMIT=2048  # MB
```

### Configuration File

Create `~/.neural_trader/backfill.yaml`:

```yaml
# Backfill configuration
defaults:
  batch_size: 10000
  workers: 10
  checkpoint: true
  memory_limit: 2048

s3:
  profile: polygon-s3
  region: us-east-1
  bucket: flatfiles
  download_path: /mnt/external/polygon_data

database:
  host: localhost
  port: 5432
  name: trading
  pool_size: 20

redis:
  host: localhost
  port: 6379
  db: 0

logging:
  level: INFO
  file: /var/log/neural_trader/backfill.log
```

## Examples

### Basic Backfill

```bash
# Download and import one month of data for AAPL
python -m data_ingestion.backfill s3 \
  --profile polygon-s3 \
  --symbols AAPL \
  --start-date 2023-01-01 \
  --end-date 2023-01-31
```

### Multiple Symbols

```bash
# Download top 10 stocks
SYMBOLS="AAPL,MSFT,GOOGL,AMZN,META,TSLA,NVDA,JPM,V,JNJ"

python -m data_ingestion.backfill s3 \
  --profile polygon-s3 \
  --symbols $SYMBOLS \
  --start-date 2023-01-01 \
  --end-date 2023-12-31 \
  --max-workers 20
```

### Resume After Failure

```bash
# Check status
python -m data_ingestion.backfill status

# Resume specific operation
python -m data_ingestion.backfill resume \
  --operation-id 2023-07-24-abc123
```

### Dry Run Mode

```bash
# Preview what would be downloaded
python -m data_ingestion.backfill s3 \
  --profile polygon-s3 \
  --symbols AAPL,MSFT \
  --start-date 2023-01-01 \
  --end-date 2023-12-31 \
  --dry-run
```

### Import from External Drive

```bash
# Import all CSV files from external drive
python -m data_ingestion.backfill file \
  --path /mnt/external/market_data \
  --format csv \
  --recursive \
  --batch-size 50000
```

## Output Formats

### Progress Output

```
Backfill Progress: AAPL (2023-01-01 to 2023-12-31)
================================================================================
Status: IN_PROGRESS
Files: 250/365 (68.5%)
Records: 2,456,789 processed
Speed: 11,234 records/sec
ETA: 00:45:23

Current: 2023-09-15.csv.gz [######### ] 78.3% (8,932/11,400 records)
Errors: 12 (0.0005%)
Memory: 1.2 GB / 2.0 GB
================================================================================
```

### JSON Status Output

```json
{
  "operation_id": "2023-07-24-abc123",
  "status": "IN_PROGRESS",
  "start_time": "2023-07-24T10:00:00Z",
  "symbols": ["AAPL", "MSFT"],
  "progress": {
    "files_total": 365,
    "files_completed": 250,
    "records_processed": 2456789,
    "records_per_second": 11234,
    "errors": 12,
    "error_rate": 0.0005
  },
  "eta": "2023-07-24T10:45:23Z"
}
```

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `AuthenticationError` | Invalid AWS credentials | Check AWS profile |
| `RateLimitError` | Too many requests | Reduce workers |
| `ValidationError` | Bad data format | Check file format |
| `DatabaseError` | Connection failed | Verify DB settings |
| `CheckpointError` | Corrupt checkpoint | Use --force flag |

### Error Recovery

```bash
# Retry failed files
python -m data_ingestion.backfill retry \
  --operation-id abc123 \
  --max-retries 3

# Skip problematic files
python -m data_ingestion.backfill resume \
  --operation-id abc123 \
  --skip-errors
```

## Performance Tuning

### Optimize for Speed

```bash
# Maximum performance settings
python -m data_ingestion.backfill s3 \
  --profile polygon-s3 \
  --symbols AAPL \
  --start-date 2023-01-01 \
  --end-date 2023-12-31 \
  --max-workers 20 \
  --batch-size 50000 \
  --no-checkpoint  # Faster but no resume capability
```

### Optimize for Reliability

```bash
# Conservative settings for stability
python -m data_ingestion.backfill s3 \
  --profile polygon-s3 \
  --symbols AAPL \
  --start-date 2023-01-01 \
  --end-date 2023-12-31 \
  --max-workers 5 \
  --batch-size 5000 \
  --checkpoint \
  --validate-each-batch
```

## Monitoring

### Prometheus Metrics

The CLI exposes metrics on port 8000:

```bash
# Start with metrics enabled
python -m data_ingestion.backfill s3 \
  --metrics-port 8000 \
  ...

# View metrics
curl http://localhost:8000/metrics
```

### Grafana Dashboard

Import the provided dashboard from `monitoring/backfill-dashboard.json` to visualize:
- Processing rate
- Error rate
- Memory usage
- Download speed
- Queue depth

## Troubleshooting

### Debug Mode

```bash
# Enable debug logging
export LOG_LEVEL=DEBUG

python -m data_ingestion.backfill s3 \
  --profile polygon-s3 \
  --symbols AAPL \
  --start-date 2023-01-01 \
  --end-date 2023-01-31 \
  --debug
```

### Common Issues

1. **Slow Performance**
   - Increase `--max-workers`
   - Increase `--batch-size`
   - Check network bandwidth

2. **High Memory Usage**
   - Reduce `--batch-size`
   - Reduce `--max-workers`
   - Enable streaming mode

3. **Authentication Failures**
   - Verify AWS profile: `aws s3 ls --profile polygon-s3`
   - Check credentials expiration
   - Verify IAM permissions

4. **Data Quality Issues**
   - Run validation: `backfill validate`
   - Check source file format
   - Review error logs